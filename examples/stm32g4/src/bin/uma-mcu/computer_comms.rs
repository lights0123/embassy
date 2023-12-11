use core::mem::size_of;

use bytemuck::bytes_of;
use defmt::{panic, *};
use embassy_stm32::usb::{Driver, Instance};
use embassy_time::{Duration, Ticker};
use embassy_usb::class::cdc_acm;
use embassy_usb::driver::EndpointError;

use crate::state::{self, ControllerState};
use crate::uma_protocol::{self, APIType, Faults};

const SYNC_BYTES: &[u8] = &[0xAA, 0x55];

#[derive(Debug)]
struct PacketWriter {
    buf: [u8; 32],
    number: u8,
}
impl PacketWriter {
    fn new() -> PacketWriter {
        let mut buf = [0; 32];
        buf[..SYNC_BYTES.len()].copy_from_slice(&SYNC_BYTES);
        PacketWriter { buf, number: 1 }
    }
    fn write<T: APIType>(&mut self, msg: &T) -> &[u8] {
        let mut written = SYNC_BYTES.len();
        let packet_length = SYNC_BYTES.len() + size_of::<T>() + size_of::<uma_protocol::PacketInfo>() + 1;
        let packet_info = uma_protocol::PacketInfo {
            packet_length: packet_length as u8,
            destination_addr: 0,
            source_addr: 1,
            api_index: T::index(),
            message_number: self.number,
        };
        self.number = self.number.wrapping_add(2);
        let packet_info = bytes_of(&packet_info);
        let data = bytes_of(msg);
        self.buf[written..][..packet_info.len()].copy_from_slice(packet_info);
        written += packet_info.len();
        self.buf[written..][..data.len()].copy_from_slice(data);
        written += data.len();
        // checksum
        self.buf[written] = self.buf[..written].iter().sum();
        &self.buf[..packet_length]
    }
}

pub async fn status_writer<'d, T: Instance + 'd>(
    sender: &mut cdc_acm::Sender<'d, Driver<'d, T>>,
) -> Result<(), EndpointError> {
    let mut writer = PacketWriter::new();
    let mut ticker = Ticker::every(Duration::from_millis(50));
    loop {
        let state = state::State::default();
        let faults = match state.controller.get().map(|controller| controller.state) {
            Some(ControllerState::Stopped) => Faults::ESTOP,
            Some(ControllerState::RemoteControl) => Faults::MANUAL_CONTROL,
            Some(ControllerState::Autonomous) => Faults::empty(),
            None => Faults::CONTROLLER_DISCONNECT,
        };
        let data = uma_protocol::Status {
            voltage: 0,
            temperature: 0,
            faults,
        };
        sender.write_packet(writer.write(&data)).await?;
        ticker.next().await;
    }
}

pub async fn status_task<'d, T: Instance + 'd>(
    sender: &mut cdc_acm::Sender<'d, Driver<'d, T>>,
) -> Result<(), EndpointError> {
    loop {
        sender.wait_connection().await;
        let _ = status_writer(sender).await;
        embassy_futures::yield_now().await;
    }
}
pub async fn cmd_reader<'d, T: Instance + 'd>(
    receiver: &mut cdc_acm::Receiver<'d, Driver<'d, T>>,
) -> Result<(), EndpointError> {
    let mut buf = [0; 32];
    loop {
        let size = receiver.read_packet(&mut buf).await?;
        let packet = &buf[..size];
        if size < SYNC_BYTES.len() + size_of::<uma_protocol::PacketInfo>() + 1 {
            warn!("Received invalid packet of size {}", size);
            continue;
        }
        if !packet.starts_with(SYNC_BYTES) {
            warn!("Received invalid sync bytes");
            continue;
        }
    }
}
