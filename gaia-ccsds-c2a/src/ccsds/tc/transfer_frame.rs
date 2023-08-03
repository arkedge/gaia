#![allow(clippy::identity_op)]

use std::mem;

use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, FromBytes, Unaligned};

#[bitfield(bytes = 5)]
#[derive(Debug, Default, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct PrimaryHeader {
    pub version_number: B2,
    pub bypass_flag: bool,
    pub control_command_flag: bool,
    #[skip]
    __: B2,
    pub scid: B10,
    pub vcid: B6,
    pub frame_length_raw: B10,
    pub frame_sequence_number: B8,
}

impl PrimaryHeader {
    pub const SIZE: usize = mem::size_of::<Self>();

    pub fn frame_length_in_bytes(&self) -> usize {
        self.frame_length_raw() as usize + 1
    }

    pub fn set_frame_length_in_bytes(&mut self, frame_length_in_bytes: usize) {
        self.set_frame_length_raw((frame_length_in_bytes - 1) as u16)
    }
}

pub const FECF_CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_IBM_3740);

pub const MAX_SIZE: usize = 1024;

#[cfg(test)]
mod tests {
    use zerocopy::LayoutVerified;

    use super::*;

    const CASE1: [u8; 5] = [0b01110010, 0b00011100, 0b10100110, 0b01100011, 0xDEu8];

    #[test]
    fn test_read() {
        let ph = LayoutVerified::<_, PrimaryHeader>::new(CASE1.as_slice()).unwrap();
        assert_eq!(1, ph.version_number());
        assert!(ph.bypass_flag());
        assert!(ph.control_command_flag());
        assert_eq!(0b1000011100, ph.scid());
        assert_eq!(0b101001, ph.vcid());
        assert_eq!(0b1001100011, ph.frame_length_raw());
        assert_eq!(0xDE, ph.frame_sequence_number());
    }

    #[test]
    fn test_write() {
        let mut bytes = [0u8; 5];
        let mut ph = LayoutVerified::<_, PrimaryHeader>::new(bytes.as_mut_slice()).unwrap();
        ph.set_version_number(1);
        ph.set_bypass_flag(true);
        ph.set_control_command_flag(true);
        ph.set_scid(0b1000011100);
        ph.set_vcid(0b101001);
        ph.set_frame_length_raw(0b1001100011);
        ph.set_frame_sequence_number(0xDE);
        assert_eq!(CASE1, bytes);
    }
}
