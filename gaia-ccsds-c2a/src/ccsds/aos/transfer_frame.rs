#![allow(clippy::identity_op)]

use std::{fmt::Display, mem};

use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, ByteSlice, FromBytes, LayoutVerified, Unaligned};

#[bitfield(bytes = 6)]
#[derive(Debug, Default, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct PrimaryHeader {
    pub version_number: B2,
    pub scid: B8,
    pub vcid: B6,
    pub frame_count_raw: B24,
    pub replay_flag: bool,
    #[skip]
    __: B7,
}

impl PrimaryHeader {
    pub const SIZE: usize = mem::size_of::<Self>();
}

impl PrimaryHeader {
    pub fn frame_count(&self) -> FrameCount {
        FrameCount(self.frame_count_raw())
    }

    pub fn set_frame_count(&mut self, FrameCount(raw): FrameCount) {
        self.set_frame_count_raw(raw);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameCount(u32);
impl FrameCount {
    const MAX: u32 = 0xFFFFFF;
    pub fn is_next_to(self, other: Self) -> bool {
        self == other.next()
    }

    #[must_use]
    pub fn next(self) -> Self {
        Self((self.0 + 1) & Self::MAX)
    }
}

impl Display for FrameCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct TransferFrame<B, T> {
    pub primary_header: LayoutVerified<B, PrimaryHeader>,
    pub data_unit_zone: B,
    pub trailer: LayoutVerified<B, T>,
}

impl<B, T> TransferFrame<B, T>
where
    B: ByteSlice,
    T: Unaligned,
{
    pub fn new(bytes: B) -> Option<Self> {
        let (primary_header, tail) = LayoutVerified::new_unaligned_from_prefix(bytes)?;
        let (data_unit_zone, trailer) = LayoutVerified::new_unaligned_from_suffix(tail)?;
        Some(Self {
            primary_header,
            data_unit_zone,
            trailer,
        })
    }
}

#[cfg(test)]
mod tests {
    use zerocopy::LayoutVerified;

    use super::*;

    const CASE1: [u8; 6] = [119, 129, 9, 226, 57, 0];

    #[test]
    fn test_read() {
        let ph = LayoutVerified::<_, PrimaryHeader>::new(CASE1.as_slice()).unwrap();
        assert_eq!(1, ph.version_number());
        assert_eq!(0xDE, ph.scid());
        assert_eq!(1, ph.vcid());
        assert_eq!(647737, ph.frame_count_raw());
        assert!(!ph.replay_flag());
    }

    #[test]
    fn test_write() {
        let mut bytes = [0u8; 6];
        let mut ph = LayoutVerified::<_, PrimaryHeader>::new(bytes.as_mut_slice()).unwrap();
        ph.set_version_number(1);
        ph.set_scid(0xDE);
        ph.set_vcid(1);
        ph.set_frame_count_raw(647737);
        ph.set_replay_flag(false);
        assert_eq!(CASE1, bytes);
    }
}
