#![allow(clippy::identity_op)]
use std::mem;

use anyhow::{anyhow, ensure, Result};
use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, FromBytes, LayoutVerified, Unaligned};

use crate::ccsds::space_packet::{self, SpacePacket};

#[bitfield(bytes = 2)]
#[derive(Debug, Default, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct Header {
    #[skip]
    __: B5,
    pub first_header_pointer_raw: B11,
}

impl Header {
    pub const SIZE: usize = mem::size_of::<Self>();
}

impl Header {
    pub fn first_header_pointer(&self) -> FirstHeaderPointer {
        self.first_header_pointer_raw()
            .try_into()
            .expect("first_header_pointer_raw must be 11bits")
    }

    pub fn set_first_header_pointer(&mut self, first_header_pointer: FirstHeaderPointer) {
        self.set_first_header_pointer_raw(first_header_pointer.into());
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum FirstHeaderPointer {
    Pointer(u16),
    NoPacketStarts,
    IdleData,
}

impl FirstHeaderPointer {
    pub const ALL_ONES: u16 = 0b11111111111;
    pub const ALL_ONES_MINUS_ONE: u16 = 0b11111111110;
}

impl TryFrom<u16> for FirstHeaderPointer {
    type Error = anyhow::Error;

    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        ensure!(raw <= Self::ALL_ONES, "too large first header pointer");
        match raw {
            Self::ALL_ONES => Ok(Self::NoPacketStarts),
            Self::ALL_ONES_MINUS_ONE => Ok(Self::IdleData),
            pointer => Ok(Self::Pointer(pointer)),
        }
    }
}

impl From<FirstHeaderPointer> for u16 {
    fn from(value: FirstHeaderPointer) -> Self {
        match value {
            FirstHeaderPointer::Pointer(pointer) => pointer,
            FirstHeaderPointer::NoPacketStarts => FirstHeaderPointer::ALL_ONES,
            FirstHeaderPointer::IdleData => FirstHeaderPointer::ALL_ONES_MINUS_ONE,
        }
    }
}

#[derive(Debug, Default)]
pub struct Defragmenter {
    buf: Vec<u8>,
}

impl Defragmenter {
    pub fn push(&mut self, m_pdu_bytes: &[u8]) -> Result<bool> {
        let (header, packet_zone) =
            LayoutVerified::<_, Header>::new_unaligned_from_prefix(m_pdu_bytes)
                .ok_or_else(|| anyhow!("given M_PDU is too small"))?;
        ensure!(
            packet_zone.len() > space_packet::PrimaryHeader::SIZE,
            "packet zone must be a Space Packet"
        );
        if self.buf.is_empty() {
            if let FirstHeaderPointer::Pointer(pointer) = header.first_header_pointer() {
                let offset = pointer as usize;
                let first_packet_bytes = &packet_zone
                    .get(offset..)
                    .ok_or_else(|| anyhow!("invalid first header pointer"))?;
                self.buf.extend_from_slice(first_packet_bytes);
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            match header.first_header_pointer() {
                FirstHeaderPointer::Pointer(_) | FirstHeaderPointer::NoPacketStarts => {
                    self.buf.extend_from_slice(packet_zone);
                    Ok(true)
                }
                FirstHeaderPointer::IdleData => Ok(false),
            }
        }
    }

    #[deprecated]
    pub fn read(&self) -> Option<SpacePacket<&'_ [u8]>> {
        let (_bytes, packet) = self.read_as_bytes_and_packet()?;
        Some(packet)
    }

    pub fn read_as_bytes_and_packet(&self) -> Option<(&[u8], SpacePacket<&'_ [u8]>)> {
        let buf = self.buf.as_slice();
        let (packet, trailer) = SpacePacket::new(buf)?;
        let bytes = &buf[..buf.len() - trailer.len()];
        Some((bytes, packet))
    }

    pub fn advance(&mut self) -> usize {
        let Some((packet, _trailer)) = SpacePacket::new(self.buf.as_slice()) else {
            return 0;
        };
        let size = packet.packet_size().expect(
            "packet_data.len() must be correct because it was constructed with ::new method",
        );
        self.buf.drain(..size);
        size
    }

    pub fn reset(&mut self) {
        self.buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut defrag = Defragmenter::default();
        let m_pdu1 = {
            let mut bytes = [0u8; Header::SIZE + space_packet::PrimaryHeader::SIZE + 1];
            let (mut m_pdu_hdr, pz) =
                LayoutVerified::<_, Header>::new_unaligned_from_prefix(bytes.as_mut_slice())
                    .unwrap();
            m_pdu_hdr.set_first_header_pointer_raw(0);
            let (mut ph, ud) =
                LayoutVerified::<_, space_packet::PrimaryHeader>::new_unaligned_from_prefix(pz)
                    .unwrap();
            ph.set_packet_data_length_in_bytes(1);
            ud[0] = 0xde;
            bytes
        };
        defrag.push(&m_pdu1).unwrap();
        let packet = defrag.read_as_bytes_and_packet().unwrap().1;
        assert_eq!(1, packet.packet_data.len());
        assert_eq!(0xDE, packet.packet_data[0]);
        let size = packet.packet_size().unwrap();
        assert_eq!(defrag.advance(), size);
        assert!(defrag.read_as_bytes_and_packet().is_none());
    }
}
