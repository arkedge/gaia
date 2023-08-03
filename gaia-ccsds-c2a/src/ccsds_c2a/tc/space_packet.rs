#![allow(clippy::identity_op)]

use std::mem;

use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, ByteSliceMut, FromBytes, LayoutVerified, Unaligned};

pub use crate::ccsds::space_packet::*;

#[bitfield(bytes = 9)]
#[derive(Debug, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct SecondaryHeader {
    pub version_number: B8,
    pub command_type: B8,
    pub command_id: B16,
    pub destination_type: B4,
    pub execution_type: B4,
    pub time_indicator: B32,
}

impl SecondaryHeader {
    pub const SIZE: usize = mem::size_of::<Self>();
}

impl Default for SecondaryHeader {
    fn default() -> Self {
        // https://github.com/ut-issl/c2a-core/blob/577e7cd148f8b5284c1b320866875fb076f52561/Docs/Core/communication.md#%E5%90%84%E3%83%95%E3%82%A3%E3%83%BC%E3%83%AB%E3%83%89%E3%81%AE%E8%AA%AC%E6%98%8E-2
        SecondaryHeader::new().with_version_number(1)
    }
}

pub struct Builder<B> {
    primary_header: LayoutVerified<B, PrimaryHeader>,
    secondary_header: LayoutVerified<B, SecondaryHeader>,
    user_data: B,
}

impl<B> Builder<B>
where
    B: ByteSliceMut,
{
    pub fn new(bytes: B) -> Option<Self> {
        let (primary_header, tail) = LayoutVerified::new_unaligned_from_prefix(bytes)?;
        let (secondary_header, user_data) = LayoutVerified::new_unaligned_from_prefix(tail)?;
        Some(Self {
            primary_header,
            secondary_header,
            user_data,
        })
    }

    pub fn ph_mut(&mut self) -> &mut PrimaryHeader {
        &mut self.primary_header
    }

    pub fn sh_mut(&mut self) -> &mut SecondaryHeader {
        &mut self.secondary_header
    }

    pub fn user_data_mut(&mut self) -> &mut B {
        &mut self.user_data
    }

    pub fn use_default(&mut self) {
        let ph = self.ph_mut();
        ph.set_packet_type(PacketType::Telecommand);
        ph.set_secondary_header_flag(true);
        ph.set_sequence_flag(SequenceFlag::Unsegmented);
        let sh = self.sh_mut();
        sh.set_version_number(1);
    }

    pub fn finish(mut self, user_data_len: usize) -> usize {
        let packet_data_len = mem::size_of::<SecondaryHeader>() + user_data_len;
        self.ph_mut()
            .set_packet_data_length_in_bytes(packet_data_len);
        mem::size_of::<PrimaryHeader>() + packet_data_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_secondary_header() {
        let mut sh = SecondaryHeader::default();
        sh.set_version_number(1);
        sh.set_command_type(0);
        sh.set_command_id(0xDEAD);
        sh.set_destination_type(1);
        sh.set_execution_type(6);
        sh.set_time_indicator(0xC001CAFE);
        let expected = [1u8, 0, 0xDE, 0xAD, 0b0001_0110, 0xC0, 0x01, 0xCA, 0xFE];
        assert_eq!(sh.as_bytes(), expected);
    }

    #[test]
    fn test_parse_secondary_header() {
        let bytes = [1u8, 0, 0xDE, 0xAD, 0b0001_0110, 0xC0, 0x01, 0xCA, 0xFE];
        let sh = SecondaryHeader::read_from(bytes.as_slice()).unwrap();
        assert_eq!(sh.version_number(), 1);
        assert_eq!(sh.command_type(), 0);
        assert_eq!(sh.command_id(), 0xDEAD);
        assert_eq!(sh.destination_type(), 1);
        assert_eq!(sh.execution_type(), 6);
        assert_eq!(sh.time_indicator(), 0xC001CAFE);
    }
}
