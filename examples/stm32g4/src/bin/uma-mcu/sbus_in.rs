use defmt::*;
use embassy_stm32::usart::{Config, DataBits, Parity, StopBits, UartRx};
use embassy_time::Instant;

use crate::interrupts::Irqs;
use crate::state::{Controller, State};

#[embassy_executor::task]
pub async fn do_status(p: crate::SbusResources, state: &'static State) {
    let mut config = Config::default();
    config.baudrate = 100_000;
    config.data_bits = DataBits::DataBits8;
    config.stop_bits = StopBits::STOP2;
    config.parity = Parity::ParityEven;
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
            Ok(bytes) => decoder.push_bytes(&buf[..bytes + 1]),
            Err(e) => warn!("Failed sbus reading: {}", e),
        }

        if let Some(packet) = decoder.try_parse() {
            debug!("Received sbus packet!");
            state.controller.set((!packet.failsafe).then(|| Controller {
                state: crate::state::ControllerState::Stopped,
                left: 1500,
                right: 1500,
                last_updated: Instant::now(),
            }));
        }

        // in case we somehow end up in an infinite loop, let other tasks run
        embassy_futures::yield_now().await;
    }
}
