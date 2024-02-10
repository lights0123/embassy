use core::ops::RangeInclusive;

use defmt::*;
use embassy_stm32::usart::{Config, DataBits, Parity, StopBits, UartRx};
use embassy_time::Instant;

use crate::interrupts::Irqs;
use crate::state::{Controller, ControllerState, State};

const SIGNAL_LOW: u16 = 172;
const SIGNAL_HIGH: u16 = 1811;
const OUT_SIGNAL_LOW: u16 = 1100;
const OUT_SIGNAL_HIGH: u16 = 1900;
const OUT_SIGNAL_MID: u16 = 1500;
const DEADZONE: u16 = 50;
const DEADZONE_RANGE: RangeInclusive<u16> = (OUT_SIGNAL_MID - DEADZONE)..=(OUT_SIGNAL_MID + DEADZONE);

fn get_pwm_value(value: u16) -> u16 {
    ((value.saturating_sub(SIGNAL_LOW)) as u32 * (OUT_SIGNAL_HIGH - OUT_SIGNAL_LOW) as u32
        / (SIGNAL_HIGH - SIGNAL_LOW) as u32) as u16
        + OUT_SIGNAL_LOW
}

#[embassy_executor::task]
pub async fn do_status(p: crate::SbusResources, state: &'static State) {
    let mut config = Config::default();
    config.baudrate = 100_000;
    config.data_bits = DataBits::DataBits8;
    config.stop_bits = StopBits::STOP2;
    config.parity = Parity::ParityEven;
    config.invert_rx = true;
    let mut rx = unwrap!(UartRx::new(p.usart, Irqs, p.sbus, p.dma, config));
    let mut decoder = sbus::SBusPacketParser::new();
    let mut buf = [0; 25];

    let mut has_received_sbus = false;
    let mut override_autonomous = false;
    loop {
        match rx.read_until_idle(&mut buf).await {
            Ok(bytes) => {
                trace!("Read {} sbus bytes", bytes);
                decoder.push_bytes(&buf[..bytes])
            }
            Err(e) => warn!("Failed sbus reading: {}", e),
        }

        if let Some(packet) = decoder.try_parse() {
            if !has_received_sbus {
                has_received_sbus = true;
                info!("Got first sbus packet!");
            }
            trace!(
                "Received sbus packet failsafe = {} dropped_frame = {}, ch5 = {}!",
                packet.failsafe,
                packet.frame_lost,
                packet.channels[4]
            );
            state.controller.set((!packet.failsafe).then(|| {
                let mut state = match packet.channels[4] {
                    ..=900 => ControllerState::Stopped,
                    ..=1400 => ControllerState::RemoteControl,
                    _ => ControllerState::Autonomous,
                };
                let left = get_pwm_value(packet.channels[5]);
                let right = get_pwm_value(packet.channels[6]);
                let mut hbridge = get_pwm_value(packet.channels[3]);
                if DEADZONE_RANGE.contains(&hbridge) {
                    hbridge = OUT_SIGNAL_MID;
                }
                if state != ControllerState::Autonomous {
                    override_autonomous = false;
                } else if override_autonomous {
                    state = ControllerState::RemoteControl;
                } else if !DEADZONE_RANGE.contains(&left) || !DEADZONE_RANGE.contains(&right) {
                    override_autonomous = true;
                    state = ControllerState::RemoteControl;
                }
                Controller {
                    state,
                    left,
                    right,
                    hbridge,
                    waterblast: packet.channels[0] > 1400,
                    flywheel: packet.channels[1] > 1400,
                    last_updated: Instant::now(),
                }
            }));
        }

        // in case we somehow end up in an infinite loop, let other tasks run
        embassy_futures::yield_now().await;
    }
}
