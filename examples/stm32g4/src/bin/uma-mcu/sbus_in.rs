use defmt::*;
use embassy_stm32::usart::{Config, DataBits, Parity, StopBits, UartRx};
use embassy_time::Instant;

use crate::interrupts::Irqs;
use crate::state::{Controller, ControllerState, State};

const SIGNAL_LOW: u16 = 172;
const SIGNAL_HIGH: u16 = 1811;
const OUT_SIGNAL_LOW: u16 = 1100;
const OUT_SIGNAL_HIGH: u16 = 1900;

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
    let mut rx = unwrap!(UartRx::new(p.USART1, Irqs, p.sbus, p.dma, config));
    let mut decoder = sbus::SBusPacketParser::new();
    let mut buf = [0; 25];

    loop {
        // wait to receive 1 full byte before waiting for idle so we're not
        // busy looping
        let err = match rx.read(&mut buf[..1]).await {
            Ok(()) => rx.read_until_idle(&mut buf[1..]).await,
            Err(e) => Err(e),
        };
        match err {
            Ok(bytes) => {
                trace!("Read {} bytes", bytes);
                decoder.push_bytes(&buf[..bytes + 1])
            }
            Err(e) => warn!("Failed sbus reading: {}", e),
        }

        if let Some(packet) = decoder.try_parse() {
            debug!(
                "Received sbus packet failsafe = {} dropped_frame = {}, ch5 = {}!",
                packet.failsafe, packet.frame_lost, packet.channels[4]
            );
            state.controller.set((!packet.failsafe).then(|| Controller {
                state: match packet.channels[4] {
                    ..=900 => ControllerState::Stopped,
                    ..=1400 => ControllerState::RemoteControl,
                    _ => ControllerState::Autonomous,
                },
                left: get_pwm_value(packet.channels[5]),
                right: get_pwm_value(packet.channels[6]),
                last_updated: Instant::now(),
            }));
        }

        // in case we somehow end up in an infinite loop, let other tasks run
        embassy_futures::yield_now().await;
    }
}
