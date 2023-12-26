use defmt::*;
use embassy_stm32::usart::{Config, UartRx};
use embassy_time::Instant;

use crate::interrupts::Irqs;
use crate::state::{Controller, ControllerState, State};

mod out;

const POLL_HEADER: u8 = 0x7E;

pub fn get_requested_id(buf: &[u8]) -> Option<u8> {
    #[deny(unused_variables)]
    match buf {
        [.., POLL_HEADER, id] => Some(*id),
        _ => None,
    }
}

#[embassy_executor::task]
pub async fn do_status(p: crate::SbusResources, state: &'static State) {
    let mut config = Config::default();
    config.baudrate = 57600;
    config.invert_rx = true;
    let mut rx = unwrap!(UartRx::new(p.USART1, Irqs, p.sbus, p.dma, config));
    let mut buf = [0; 14];

    loop {
        // in case we somehow end up in an infinite loop, let other tasks run
        embassy_futures::yield_now().await;
        let buf = match rx.read_until_idle(&mut buf).await {
            Ok(bytes) => {
                trace!("Read {} sport bytes", bytes);
                &buf[..bytes]
            }
            Err(e) => {
                warn!("Failed sbus reading: {}", e);
                continue;
            }
        };

        let Some(sport_id) = get_requested_id(buf) else {
            continue;
        };

        match sport_id {
            _ => {}
        }
    }
}
