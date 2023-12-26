/// * sensor ID(s)   | FRSKY_SP_GPS_COURSE ~ FRSKY_SP_GPS_COURSE+15 (0x0840 ~ 0x084f)
/// * physical ID(s) | 4 - GPS
/// * value          | (int) float * 100 [°]
/// * limits         | 0~359.99°
pub const GPS_HEADING: u16 = 0x0840;

/// * info | comment
/// * ---- | -------
/// * sensor ID(s)   | FRSKY_SP_GPS_SPEED ~ FRSKY_SP_GPS_SPEED+15 (0x0830 ~ 0x083f)
/// * physical ID(s) | 4 - GPS
/// * value          | (int) float * 1000 (kts)
/// *
/// * \brief GPS speed
/// * \warning The speed shown on OpenTX has a little drift, because the knots to shown value conversion is simplified.
/// * Allthough, raw knots will be recorded in the logs, and the conversion will be correctly in Companion.
/// * This was discussed in this issue: https://github.com/opentx/opentx/issues/1422
/// */
pub const GPS_SPEED: u16 = 0x0830;

#[derive(Default)]
pub struct SportSensorReading {
    buf: [u8; 12],
}

impl SportSensorReading {
    fn write(&mut self, mut c: u8, written: &mut usize, crc: &mut u16) {
        if matches!(c, 0x7D | 0x7E) {
            self.buf[*written] = 0x7D;
            *written += 1;
            c ^= 0x20;
        }

        self.buf[*written] = c;
        *written += 1;

        *crc += c as u16;
        *crc += *crc >> 8;
        *crc &= 0x00FF;
    }

    pub fn encode(&mut self, id: u16, val: u32) -> &[u8] {
        let mut crc = 0;
        let mut written = 0;

        // type, fixed
        self.write(0x10, &mut written, &mut crc);

        for c in id.to_le_bytes() {
            self.write(c, &mut written, &mut crc);
        }

        for c in val.to_le_bytes() {
            self.write(c, &mut written, &mut crc);
        }

        self.write(0xFF - crc as u8, &mut written, &mut crc);

        &self.buf[..written]
    }
}
