#![allow(clippy::identity_op)]

use std::mem;

use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified, Unaligned};

use crate::ccsds;
pub use crate::ccsds::space_packet::*;

#[bitfield(bytes = 20)]
#[derive(Debug, Clone, FromBytes, AsBytes, Unaligned, Default)]
#[repr(C)]
pub struct SecondaryHeader {
    pub version_number: B8,
    pub board_time: B32,
    pub telemetry_id: B8,
    pub global_time_bits: B64,
    pub on_board_subnetwork_time: B32,
    pub destination_flags: B8,
    pub data_recorder_partition: B8,
}

impl SecondaryHeader {
    pub const SIZE: usize = mem::size_of::<Self>();

    const HOUSEKEEPING: u8 = 0b00000001;
    const MISSION: u8 = 0b00000010;
    #[deprecated = "this is not a generic method. it's better to build a proper routing mechanism using destination_flags directly"]
    pub fn is_realtime(&self) -> bool {
        self.destination_flags() & (Self::HOUSEKEEPING | Self::MISSION) != 0
    }

    pub fn global_time(&self) -> f64 {
        f64::from_bits(self.global_time_bits())
    }
}

#[derive(Debug)]
pub struct SpacePacket<B: ByteSlice> {
    pub primary_header: LayoutVerified<B, PrimaryHeader>,
    pub secondary_header: LayoutVerified<B, SecondaryHeader>,
    pub user_data: B,
}

impl<B> SpacePacket<B>
where
    B: ByteSlice,
{
    pub fn from_generic(generic: ccsds::SpacePacket<B>) -> Option<Self> {
        let (secondary_header, user_data) =
            LayoutVerified::<_, SecondaryHeader>::new_unaligned_from_prefix(generic.packet_data)?;
        Some(Self {
            primary_header: generic.primary_header,
            secondary_header,
            user_data,
        })
    }
}
