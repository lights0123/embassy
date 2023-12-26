use defmt::*;
use embassy_stm32::gpio::{Level, Output, OutputOpenDrain, Pull, Speed};
use embassy_stm32::usart::{Config, Uart, UartRx};
use embassy_time::Instant;

use self::out::SportSensorReading;
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
pub async fn do_status(p: crate::SportResources, state: &'static State) {
    let mut config = Config::default();
    config.baudrate = 57600;
    config.invert_rx = true;
    config.invert_tx = true;
    let mut uart = unwrap!(Uart::new_half_duplex_on_rx(
        p.usart, p.sport, Irqs, p.dma_1, p.dma_2, config
    ));
    let mut buf = [0; 14];
    let mut out_buf = SportSensorReading::default();
    let start = Instant::now();

    loop {
        // in case we somehow end up in an infinite loop, let other tasks run
        embassy_futures::yield_now().await;
        let buf = match uart.read_until_idle(&mut buf).await {
            Ok(bytes) => {
                trace!("Read {} sport bytes", bytes);
                &buf[..bytes]
            }
            Err(e) => {
                warn!("Failed sport reading: {}", e);
                continue;
            }
        };

        let Some(sport_id) = get_requested_id(buf) else {
            continue;
        };

        trace!("sport request {}", sport_id);

        match sport_id {
            // Physical ID 4 - GPS / altimeter (normal precision)
            0x83 => {
                let val = start.elapsed().as_millis() / 10 % 100;
                let buf = out_buf.encode(out::GPS_SPEED, (val as f32 / 1.852 * 1000.0) as u32);
                if let Err(e) = uart.write(buf).await {
                    warn!("failed to write sport buf of len {}: {}", buf.len(), e);
                }
            }
            _ => {}
        }
    }
}
