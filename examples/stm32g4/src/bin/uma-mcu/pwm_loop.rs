use defmt::*;
use embassy_stm32::gpio::{Level, Output, OutputType, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::timer::{CaptureCompare16bitInstance, Channel};
use embassy_stm32::wdg::IndependentWatchdog;
use embassy_time::{Duration, Instant, Ticker};

use crate::config::{MAX_COMPUTER_TIMEOUT, MAX_RC_TIMEOUT};
use crate::state::{ControllerState, State};

const SERVO_PWM_FREQ: Hertz = Hertz::hz(50);
const REST_PWM_VALUE: u16 = 1500;

fn us_to_duty<T: CaptureCompare16bitInstance>(pwm: &SimplePwm<T>, us: u16) -> u16 {
    let us = us.clamp(1100, 1900);
    let max = pwm.get_max_duty();
    let period_width_us = 1_000_000 / SERVO_PWM_FREQ.0;
    let val = ((us as u32) * (max as u32) / period_width_us) as u16;
    trace!("Converted {}us to {} duty cycle / {}", us, val, max);
    val
}
fn us_to_duty_full<T: CaptureCompare16bitInstance>(pwm: &SimplePwm<T>, us: u16) -> u16 {
    let us = (us.clamp(1100, 1900) as i16 - REST_PWM_VALUE as i16).abs();
    let max = pwm.get_max_duty();
    let val = ((us as u32) * (max as u32) / 400) as u16;
    trace!("Converted {}us to {} duty cycle / {}", us, val, max);
    val
}
fn set_pwm_us<T: CaptureCompare16bitInstance>(pwm: &mut SimplePwm<T>, channel: Channel, us: u16) {
    pwm.set_duty(channel, us_to_duty(pwm, us));
}
fn set_pwm_us_full<T: CaptureCompare16bitInstance>(pwm: &mut SimplePwm<T>, channel: Channel, us: u16) {
    pwm.set_duty(channel, us_to_duty_full(pwm, us));
}

#[embassy_executor::task]
pub async fn do_status(p: crate::OutResources, state: &'static State) {
    let mut motor_enable = Output::new(p.motor_enable, Level::Low, Speed::Low);
    let mut timer = Ticker::every(Duration::from_hz(SERVO_PWM_FREQ.0 as u64));
    let mut water_blast = Output::new(p.waterblast, Level::Low, Speed::Low);
    let left_signal = PwmPin::new_ch1(p.left_motor, OutputType::PushPull);
    let right_signal = PwmPin::new_ch2(p.right_motor, OutputType::PushPull);
    let hbridge_left = PwmPin::new_ch1(p.hbridge_left, OutputType::PushPull);
    let hbridge_right = PwmPin::new_ch1(p.hbridge_right, OutputType::PushPull);
    let mut left_pwm = SimplePwm::new(
        p.TIM1,
        Some(left_signal),
        None,
        None,
        None,
        SERVO_PWM_FREQ,
        Default::default(),
    );
    let mut right_pwm = SimplePwm::new(
        p.TIM2,
        Some(hbridge_left),
        Some(right_signal),
        None,
        None,
        SERVO_PWM_FREQ,
        Default::default(),
    );
    let mut hbridge_right_pwm = SimplePwm::new(
        p.TIM16,
        Some(hbridge_right),
        None,
        None,
        None,
        SERVO_PWM_FREQ,
        Default::default(),
    );
    set_pwm_us(&mut left_pwm, Channel::Ch1, REST_PWM_VALUE);
    set_pwm_us(&mut right_pwm, Channel::Ch2, REST_PWM_VALUE);
    let mut wdt = IndependentWatchdog::new(p.IWDG, 100 * 1000);
    wdt.unleash();
    left_pwm.enable(Channel::Ch1);
    right_pwm.enable(Channel::Ch2);

    loop {
        let now = Instant::now();
        let control_signal = state.controller.get().and_then(|rc| match rc.state {
            _ if now - rc.last_updated > MAX_RC_TIMEOUT => None,
            ControllerState::Stopped => None,
            ControllerState::RemoteControl => Some((rc.left, rc.right, rc.hbridge, rc.waterblast)),
            ControllerState::Autonomous => {
                let computer_control = state.computer.get();
                if now - computer_control.last_updated > MAX_COMPUTER_TIMEOUT {
                    Some((REST_PWM_VALUE, REST_PWM_VALUE, REST_PWM_VALUE, false))
                } else {
                    Some((
                        computer_control.left,
                        computer_control.right,
                        computer_control.hbridge,
                        computer_control.waterblast,
                    ))
                }
            }
        });
        trace!("pwm loop: {}", control_signal);
        if let Some((left, right, hbridge, waterblast)) = control_signal {
            motor_enable.set_high();
            set_pwm_us(&mut left_pwm, Channel::Ch1, left);
            set_pwm_us(&mut right_pwm, Channel::Ch2, right);
            if hbridge > REST_PWM_VALUE {
                set_pwm_us_full(&mut right_pwm, Channel::Ch1, hbridge);
                hbridge_right_pwm.set_duty(Channel::Ch1, 0);
            } else {
                set_pwm_us_full(&mut hbridge_right_pwm, Channel::Ch1, hbridge);
                right_pwm.set_duty(Channel::Ch1, 0);
            }
            water_blast.set_level(waterblast.into());
        } else {
            motor_enable.set_low();
            set_pwm_us(&mut left_pwm, Channel::Ch1, REST_PWM_VALUE);
            set_pwm_us(&mut right_pwm, Channel::Ch2, REST_PWM_VALUE);
            right_pwm.set_duty(Channel::Ch1, 0);
            hbridge_right_pwm.set_duty(Channel::Ch1, 0);
            water_blast.set_low();
        }
        wdt.pet();
        timer.next().await;
    }
}
