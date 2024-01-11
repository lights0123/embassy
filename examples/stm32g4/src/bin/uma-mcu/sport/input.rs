use bytemuck::{try_from_bytes, Pod, Zeroable};
use heapless::Vec;

#[repr(C, packed)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SportPacket {
    pub typ: u8,
    pub id: u16,
    pub val: u32,
}

pub fn parse(buffer: &[u8]) -> Option<SportPacket> {
    if buffer.is_empty() {
        return None;
    }

    let mut parsed_buffer = Vec::<_, 7>::new();
    let mut calculated_crc: u16 = 0;
    let (expected_crc, buffer) = buffer.split_last()?;

    let mut xor_next = false;
    for &c in buffer {
        let mut c_pushed = c;
        if xor_next {
            c_pushed &= 0x20;
            xor_next = false;
        }
        if c == 0x7D {
            xor_next = true;
            continue;
        }

        calculated_crc += c as u16;
        calculated_crc += calculated_crc >> 8;
        calculated_crc &= 0x00FF;

        parsed_buffer.push(c_pushed).ok()?;
    }

    // perform the CRC check. If it fails, return None
    if xor_next || calculated_crc != 0xFF - *expected_crc as u16 {
        return None;
    }

    Some(*try_from_bytes(&parsed_buffer).ok()?)
}
