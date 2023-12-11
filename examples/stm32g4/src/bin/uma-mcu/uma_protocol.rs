use bytemuck::{Pod, Zeroable};

pub trait APIType: Pod {
    fn index() -> u8;
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct SetPWMOut {
    pub update: u8,
    pub outputs: [i16; 8],
}
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, Pod, Zeroable)]
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
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Status {
    pub voltage: u8,
    pub temperature: u8,
    pub faults: Faults,
}
impl APIType for Status {
    fn index() -> u8 {
        255
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct PacketInfo {
    pub packet_length: u8,
    pub destination_addr: u8,
    pub source_addr: u8,
    pub api_index: u8,
    pub message_number: u8,
}
