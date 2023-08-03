#![allow(clippy::identity_op)]

use std::mem;

use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified, Unaligned};

#[bitfield(bytes = 6)]
#[derive(Debug, FromBytes, AsBytes, Unaligned, Default)]
#[repr(C)]
pub struct PrimaryHeader {
    pub version_number: B3,
    pub packet_type: PacketType,
    pub secondary_header_flag: bool,
    pub apid: B11,
    pub sequence_flag: SequenceFlag,
    pub sequence_count: B14,
    pub packet_data_length_raw: B16,
}

impl PrimaryHeader {
    pub const SIZE: usize = mem::size_of::<Self>();

    pub fn packet_data_length_in_bytes(&self) -> usize {
        self.packet_data_length_raw() as usize + 1
    }

    pub fn set_packet_data_length_in_bytes(&mut self, packet_data_length_in_bytes: usize) {
        assert!(packet_data_length_in_bytes > 0);
        self.set_packet_data_length_raw(packet_data_length_in_bytes as u16 - 1);
    }

    pub fn is_idle_packet(&self) -> bool {
        // > 4.1.3.3.4.4 For Idle Packets the APID shall be ‘11111111111’,
        // > that is, ‘all ones’(see reference [4]).
        // ref: https://public.ccsds.org/Pubs/133x0b2e1.pdf
        const ALL_ONES_11BIT: u16 = 0b11111111111;
        self.apid() == ALL_ONES_11BIT
    }
}

#[derive(Debug)]
pub struct SpacePacket<B: ByteSlice> {
    pub primary_header: LayoutVerified<B, PrimaryHeader>,
    pub packet_data: B,
}

impl<B> SpacePacket<B>
where
    B: ByteSlice,
{
    pub fn new(bytes: B) -> Option<(SpacePacket<B>, B)> {
        let (primary_header, tail) =
            LayoutVerified::<_, PrimaryHeader>::new_unaligned_from_prefix(bytes)?;
        let pd_size = primary_header.packet_data_length_in_bytes();
        if tail.len() < pd_size {
            return None;
        }
        let (packet_data, trailer) = tail.split_at(pd_size);
        let space_packet = SpacePacket {
            primary_header,
            packet_data,
        };
        debug_assert!(space_packet.packet_size().is_some());
        Some((space_packet, trailer))
    }

    /// returns None if the packet data length field in PH
    /// is not matched with the actual length of packet_data
    pub fn packet_size(&self) -> Option<usize> {
        let len_in_ph = self.primary_header.packet_data_length_in_bytes();
        if self.packet_data.len() == len_in_ph {
            Some(PrimaryHeader::SIZE + len_in_ph)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BitfieldSpecifier)]
#[bits = 1]
pub enum PacketType {
    Telemetry = 0,
    Telecommand = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BitfieldSpecifier)]
#[bits = 2]
pub enum SequenceFlag {
    Continuation = 0b00,
    First = 0b01,
    Last = 0b10,
    Unsegmented = 0b11,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_primary_header() {
        let mut ph = PrimaryHeader::default();
        ph.set_version_number(6);
        ph.set_packet_type(PacketType::Telecommand);
        ph.set_secondary_header_flag(true);
        ph.set_apid(2000);
        ph.set_sequence_flag(SequenceFlag::First);
        ph.set_sequence_count(16000);
        ph.set_packet_data_length_in_bytes(0xABCD);
        let actual = ph.as_bytes();
        let expected = [
            0b1101_1111,
            0b1101_0000,
            0b0111_1110,
            0b1000_0000,
            0xAB,
            0xCC,
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parse_primary_header() {
        let bytes = [
            0b1101_1111,
            0b1101_0000,
            0b0111_1110,
            0b1000_0000,
            0xAB,
            0xCC,
        ];
        let ph = PrimaryHeader::read_from(bytes.as_slice()).unwrap();
        assert_eq!(ph.version_number(), 6);
        assert_eq!(ph.packet_type(), PacketType::Telecommand);
        assert!(ph.secondary_header_flag());
        assert_eq!(ph.apid(), 2000);
        assert_eq!(ph.sequence_flag(), SequenceFlag::First);
        assert_eq!(ph.sequence_count(), 16000);
        assert_eq!(ph.packet_data_length_in_bytes(), 0xABCD);
    }
}
