#![allow(clippy::identity_op)]

use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, FromBytes, Unaligned};

#[bitfield(bytes = 1)]
#[derive(Debug, Default, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct Header {
    pub sequence_flag: SequenceFlag,
    pub map_id: B6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BitfieldSpecifier)]
#[bits = 2]
pub enum SequenceFlag {
    Continuing = 0b00,
    First = 0b01,
    Last = 0b10,
    NoSegmentation = 0b11,
}
