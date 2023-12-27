use bytemuck::{bytes_of, try_from_bytes, Pod, Zeroable};
use defmt::Format;

pub trait APIType: Pod {
    const INDEX: u8;

    fn encode(&self) -> &[u8] {
        bytes_of(self)
    }

    fn decode(buf: &[u8]) -> Option<&Self> {
        try_from_bytes(buf).ok()
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SetPWMOut {
    pub update: u8,
    pub outputs: [u16; 8],
}
impl APIType for SetPWMOut {
    const INDEX: u8 = 246;
}

#[repr(C, packed)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SetGpsStats {
    pub speed: u32,
    pub heading: u32,
}
impl APIType for SetGpsStats {
    const INDEX: u8 = 230;
}
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    pub struct Faults: u8 {
        const ESTOP = 0b00000001;
        const UNDERVOLTAGE = 0b00000010;
        const OVERVOLTAGE = 0b00000100;
        const OVERTEMPERATURE = 0b00001000;
        const MANUAL_CONTROL = 0b00010000;
        const RELAY_WELD = 0b00100000;
        const CONTROLLER_DISCONNECT = 0b01000000;
    }
}
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Status {
    pub voltage: u8,
    pub temperature: u8,
    pub faults: Faults,
}
impl APIType for Status {
    const INDEX: u8 = 255;
}
#[repr(C)]
#[derive(Format, Copy, Clone, Pod, Zeroable)]
pub struct PacketInfo {
    pub packet_length: u8,
    pub destination_addr: u8,
    pub source_addr: u8,
    pub api_index: u8,
    pub message_number: u8,
}
