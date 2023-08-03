use std::{
    mem,
    ops::{Deref, DerefMut},
};

use zerocopy::{ByteSlice, ByteSliceMut, LayoutVerified};

pub use crate::ccsds::tc::transfer_frame::*;

pub const TRAILER_SIZE: usize = 2;

pub struct Builder<B> {
    bare: B,
}

impl<B> Builder<B>
where
    B: ByteSliceMut,
{
    pub fn new(bytes: B) -> Option<Self> {
        if bytes.len() < TRAILER_SIZE {
            None
        } else {
            Some(Self { bare: bytes })
        }
    }

    pub fn bare_mut(&mut self) -> Option<BareBuilder<&mut [u8]>> {
        let bare_len = self.bare.len();
        BareBuilder::new(&mut self.bare[..bare_len - TRAILER_SIZE])
    }

    pub fn finish(mut self, bare_len: usize) -> usize {
        let frame_len = bare_len + TRAILER_SIZE;
        assert!(frame_len <= self.bare.len());
        let bare_bytes = &self.bare[..bare_len];
        let fecw = FECF_CRC.checksum(bare_bytes);
        let fecw_bytes = &mut self.bare[bare_len..][..TRAILER_SIZE];
        fecw_bytes.copy_from_slice(&fecw.to_be_bytes());
        frame_len
    }
}

pub struct BareBuilder<B> {
    primary_header: LayoutVerified<B, PrimaryHeader>,
    data_field: B,
}

impl<B> BareBuilder<B>
where
    B: ByteSlice,
{
    fn new(bytes: B) -> Option<Self> {
        let (primary_header, data_field) = LayoutVerified::new_unaligned_from_prefix(bytes)?;
        Some(Self {
            primary_header,
            data_field,
        })
    }
}

impl<B> BareBuilder<B>
where
    B: ByteSliceMut,
{
    pub fn use_default(&mut self) {
        self.set_version_number(0);
        self.set_bypass_flag(true); // Type-Bx
        self.set_control_command_flag(false); // Type-xD
        self.set_vcid(0);
    }

    pub fn data_field_mut(&mut self) -> &mut B {
        &mut self.data_field
    }

    pub fn finish(mut self, data_field_len: usize) -> usize {
        let bare_len = mem::size_of::<PrimaryHeader>() + data_field_len;
        let frame_len = bare_len + TRAILER_SIZE;
        self.set_frame_length_in_bytes(frame_len);
        bare_len
    }
}

impl<B> Deref for BareBuilder<B>
where
    B: ByteSlice,
{
    type Target = PrimaryHeader;

    fn deref(&self) -> &Self::Target {
        &self.primary_header
    }
}

impl<B> DerefMut for BareBuilder<B>
where
    B: ByteSliceMut,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.primary_header
    }
}
