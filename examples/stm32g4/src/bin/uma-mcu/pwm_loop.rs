use defmt::*;
use embassy_stm32::gpio::{Level, Output, OutputType, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::timer::{CaptureCompare16bitInstance, Channel};
use embassy_stm32::wdg::IndependentWatchdog;
use embassy_time::{Duration, Instant, Ticker};

use crate::state::{ControllerState, State};

const MAX_RC_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_COMPUTER_TIMEOUT: Duration = Duration::from_millis(1000);
const SERVO_PWM_FREQ: Hertz = Hertz::hz(50);

fn us_to_duty<T: CaptureCompare16bitInstance>(pwm: &SimplePwm<T>, us: u16) -> u16 {
    let us = us.clamp(1100, 1900);
    let max = pwm.get_max_duty();
    let period_width_us = 1_000_000 / SERVO_PWM_FREQ.0;
    let val = ((us as u32) * (max as u32) / period_width_us) as u16;
    debug!("Converted {}us to {} duty cycle / {}", us, val, max);
    val
}
fn set_pwm_us<T: CaptureCompare16bitInstance>(pwm: &mut SimplePwm<T>, channel: Channel, us: u16) {
    pwm.set_duty(channel, us_to_duty(pwm, us));
}

#[embassy_executor::task]
pub async fn do_status(p: crate::OutResources, state: &'static State) {
    let mut motor_enable = Output::new(p.motor_enable, Level::High, Speed::Low);
    let mut timer = Ticker::every(Duration::from_hz(SERVO_PWM_FREQ.0 as u64));
    let left_signal = PwmPin::new_ch1(p.pwm_1a, OutputType::PushPull);
    let right_signal = PwmPin::new_ch2(p.pwm_2b, OutputType::PushPull);
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
        None,
        Some(right_signal),
        None,
        None,
        SERVO_PWM_FREQ,
        Default::default(),
    );
    left_pwm.enable(Channel::Ch1);
    set_pwm_us(&mut left_pwm, Channel::Ch1, 1500);
    right_pwm.enable(Channel::Ch2);
    set_pwm_us(&mut right_pwm, Channel::Ch2, 1500);
    let mut wdt = IndependentWatchdog::new(p.IWDG, 100 * 1000);
    wdt.unleash();

    loop {
        let now = Instant::now();
        let control_signal = state.controller.get().and_then(|rc| match rc.state {
            _ if now.duration_since(rc.last_updated) > MAX_RC_TIMEOUT => None,
            ControllerState::Stopped => None,
            ControllerState::RemoteControl => Some((rc.left, rc.right)),
            ControllerState::Autonomous => {
                let computer_control = state.computer.get();
                if now.duration_since(computer_control.last_updated) > MAX_COMPUTER_TIMEOUT {
                    Some((1500, 1500))
                } else {
                    Some((computer_control.left, computer_control.right))
                }
            }
        });
        trace!("pwm loop: {}", control_signal);
        if let Some((left, right)) = control_signal {
            motor_enable.set_high();
            set_pwm_us(&mut left_pwm, Channel::Ch1, left);
            set_pwm_us(&mut right_pwm, Channel::Ch2, right);
        } else {
            motor_enable.set_low();
        }
        wdt.pet();
        timer.next().await;
    }
}
