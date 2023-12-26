use defmt::*;
use embassy_stm32::usart::{Config, Uart};
use embassy_time::Duration;

use self::out::SportSensorReading;
use crate::interrupts::Irqs;
use crate::state::State;

mod out;

const POLL_HEADER: u8 = 0x7E;
const MAX_TIMEOUT: Duration = Duration::from_millis(1000);

pub fn get_requested_id(buf: &[u8]) -> Option<u8> {
    #[deny(unused_variables)]
    match buf {
        [.., POLL_HEADER, id] => Some(*id),
        _ => None,
    }
}

#[derive(Default)]
struct StatusHandler {
    last_gps_val: u8,
}

impl StatusHandler {
    fn handle(&mut self, sport_id: u8, state: &State) -> Option<(u16, u32)> {
        match sport_id {
            // Physical ID 4 - GPS / altimeter (normal precision)
            0x83 => {
                self.last_gps_val += 1;
                state
                    .gps_stats
                    .get()
                    .filter(|stats| stats.last_updated.elapsed() < MAX_TIMEOUT)
                    .map(|stats| {
                        if self.last_gps_val % 2 == 0 {
                            (out::GPS_SPEED, stats.speed)
                        } else {
                            (out::GPS_HEADING, stats.heading)
                        }
                    })
            }
            _ => None,
        }
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
    let mut handler = StatusHandler::default();

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

        let output = handler.handle(sport_id, state);

        if let Some(output) = output {
            if let Err(e) = uart.write(out_buf.encode(output.0, output.1)).await {
                warn!("failed to write sport buf of len {}: {}", buf.len(), e);
            }
        }
    }
}
