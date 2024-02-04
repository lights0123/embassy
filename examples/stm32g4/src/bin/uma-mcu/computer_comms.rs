use core::mem::size_of;

use bytemuck::{bytes_of, try_from_bytes};
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::peripherals;
use embassy_time::{Duration, Instant, Ticker};
use embassy_usb::class::cdc_acm;
use embassy_usb::driver::EndpointError;
use static_cell::StaticCell;

use crate::config::MAX_RC_TIMEOUT;
use crate::state::{self, ControllerState, State};
use crate::uma_protocol::{self, APIType, Faults};

const SYNC_BYTES: &[u8] = &[0xAA, 0x55];
const MAX_PACKET_SIZE: usize = 32;

type Driver = embassy_stm32::usb::Driver<'static, peripherals::USB>;
type Sender = cdc_acm::Sender<'static, Driver>;
type Receiver = cdc_acm::Receiver<'static, Driver>;

#[derive(Debug)]
struct PacketWriter {
    buf: [u8; 32],
    number: u8,
}
impl PacketWriter {
    fn new() -> PacketWriter {
        let mut buf = [0; MAX_PACKET_SIZE];
        buf[..SYNC_BYTES.len()].copy_from_slice(&SYNC_BYTES);
        PacketWriter { buf, number: 1 }
    }
    #[must_use]
    fn write<T: APIType>(&mut self, msg: &T) -> &[u8] {
        let mut written = SYNC_BYTES.len();
        let packet_length = SYNC_BYTES.len() + size_of::<T>() + size_of::<uma_protocol::PacketInfo>() + 1;
        let packet_info = uma_protocol::PacketInfo {
            packet_length: packet_length as u8,
            destination_addr: 0,
            source_addr: 1,
            api_index: T::INDEX,
            message_number: self.number,
        };
        self.number = self.number.wrapping_add(2);
        let packet_info = bytes_of(&packet_info);
        self.buf[written..][..packet_info.len()].copy_from_slice(packet_info);
        written += packet_info.len();
        let data = msg.encode();
        self.buf[written..][..data.len()].copy_from_slice(data);
        written += data.len();
        // checksum
        self.buf[written] = self.buf[..written].iter().sum();
        &self.buf[..packet_length]
    }
}

async fn status_writer(sender: &mut Sender, state: &State) -> Result<(), EndpointError> {
    let mut writer = PacketWriter::new();
    let mut ticker = Ticker::every(Duration::from_millis(50));
    loop {
        let faults = match state
            .controller
            .get()
            .filter(|controller| controller.last_updated.elapsed() < MAX_RC_TIMEOUT)
            .map(|controller| controller.state)
        {
            Some(ControllerState::Stopped) => Faults::ESTOP,
            Some(ControllerState::RemoteControl) => Faults::MANUAL_CONTROL,
            Some(ControllerState::Autonomous) => Faults::empty(),
            None => Faults::CONTROLLER_DISCONNECT,
        };
        let data = uma_protocol::Status {
            voltage: state.sensor.voltage.get(),
            current: state.sensor.current.get(),
            temp_1: state.sensor.temp_1.get() as u8,
            temp_2: state.sensor.temp_2.get() as u8,
            faults,
        };
        let buf = writer.write(&data);
        trace!("usb write len {}", buf.len());
        sender.write_packet(buf).await?;
        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn status_task(mut sender: Sender, state: &'static State) {
    loop {
        info!("waiting for status conn");
        sender.wait_connection().await;
        info!("status USB connection!");
        let _ = status_writer(&mut sender, state).await;
        info!("status USB disconnection!");
        embassy_futures::yield_now().await;
    }
}

fn handle_pwm_out(msg: &uma_protocol::SetPWMOut, state: &State) {
    trace!("got new pwm message");
    state.computer.set(state::Computer {
        left: msg.outputs[0],
        right: msg.outputs[1],
        hbridge: msg.outputs[2],
        waterblast: msg.outputs[3] > 1500,
        last_updated: Instant::now(),
    });
}

fn handle_gps_stats(msg: &uma_protocol::SetGpsStats, state: &State) {
    trace!("got new gps message");
    state.gps_stats.set(Some(state::GpsStats {
        speed: msg.speed,
        heading: msg.heading,
        last_updated: Instant::now(),
    }));
}

fn parse_packet(packet: &[u8], state: &State) -> Option<()> {
    let full_len = packet.len();
    if full_len < SYNC_BYTES.len() + size_of::<uma_protocol::PacketInfo>() + 1 {
        warn!("Received invalid packet of size {}", packet.len());
        return None;
    }
    if !packet.starts_with(SYNC_BYTES) {
        warn!("Received invalid sync bytes");
        return None;
    }
    let packet = packet.get(SYNC_BYTES.len()..)?;

    let header: &uma_protocol::PacketInfo =
        try_from_bytes(packet.get(..size_of::<uma_protocol::PacketInfo>())?).ok()?;
    let packet = packet.get(size_of::<uma_protocol::PacketInfo>()..)?;
    if header.packet_length as usize != full_len {
        warn!(
            "Received packet of len {}, but header says len {}",
            full_len, header.packet_length
        );
        return None;
    }
    let data = packet.get(..packet.len() - 1)?;
    trace!("got new api index {} of size {}", header.api_index, data.len());
    match header.api_index {
        uma_protocol::SetPWMOut::INDEX => handle_pwm_out(APIType::decode(data)?, state),
        uma_protocol::SetGpsStats::INDEX => handle_gps_stats(APIType::decode(data)?, state),
        _ => {}
    }

    Some(())
}

async fn cmd_reader(receiver: &mut Receiver, state: &State) -> Result<(), EndpointError> {
    let mut buf = [0; MAX_PACKET_SIZE];
    loop {
        let size = receiver.read_packet(&mut buf).await?;
        let packet = &buf[..size];
        parse_packet(packet, state);
    }
}

#[embassy_executor::task]
async fn reader_task(mut rx: Receiver, state: &'static State) {
    loop {
        rx.wait_connection().await;
        info!("USB connection!");
        let _ = cmd_reader(&mut rx, state).await;
        info!("USB disconnection!");
        embassy_futures::yield_now().await;
    }
}

pub fn init_usb(p: crate::UsbResources, spawner: Spawner, shared_state: &'static State) {
    let driver = Driver::new(p.USB, crate::interrupts::Irqs, p.PA12, p.PA11);

    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("UM Autonomy");
    config.product = Some("PCB");
    config.serial_number = Some("123456");

    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;
    config.self_powered = true;

    struct UsbStatics {
        device_descriptor: [u8; 256],
        config_descriptor: [u8; 256],
        bos_descriptor: [u8; 256],
        control_buf: [u8; 64],
        state: cdc_acm::State<'static>,
    }
    static USB_STATICS: StaticCell<UsbStatics> = StaticCell::new();

    // Initialize it at runtime. This returns a `&'static mut`.
    let statics = USB_STATICS.init_with(|| UsbStatics {
        device_descriptor: [0; 256],
        config_descriptor: [0; 256],
        bos_descriptor: [0; 256],
        control_buf: [0; 64],
        state: Default::default(),
    });

    let mut builder = embassy_usb::Builder::new(
        driver,
        config,
        &mut statics.device_descriptor,
        &mut statics.config_descriptor,
        &mut statics.bos_descriptor,
        &mut [], // no msos descriptors
        &mut statics.control_buf,
    );

    let class = cdc_acm::CdcAcmClass::new(&mut builder, &mut statics.state, MAX_PACKET_SIZE as u16);

    let usb = builder.build();

    #[embassy_executor::task]
    async fn usb_bg_task(mut usb: embassy_usb::UsbDevice<'static, Driver>) {
        usb.run().await;
    }

    spawner.must_spawn(usb_bg_task(usb));

    let (sender, receiver) = class.split();

    spawner.must_spawn(reader_task(receiver, shared_state));
    spawner.must_spawn(status_task(sender, shared_state));
}
