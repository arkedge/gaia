use std::{
    mem,
    ops::{Deref, DerefMut},
};

use zerocopy::{ByteSlice, ByteSliceMut, LayoutVerified};

use crate::ccsds::tc::segment::*;

pub struct Builder<B> {
    header: LayoutVerified<B, Header>,
    body: B,
}

impl<B> Builder<B>
where
    B: ByteSlice,
{
    pub fn new(bytes: B) -> Option<Self> {
        let (header, body) = LayoutVerified::new_unaligned_from_prefix(bytes)?;
        Some(Self { header, body })
    }
}

impl<B> Builder<B>
where
    B: ByteSliceMut,
{
    pub fn use_default(&mut self) {
        self.set_map_id(0b10);
        self.set_sequence_flag(SequenceFlag::NoSegmentation);
    }

    pub fn body_mut(&mut self) -> &mut B {
        &mut self.body
    }

    pub fn finish(self, body_len: usize) -> usize {
        mem::size_of::<Header>() + body_len
    }
}

impl<B> Deref for Builder<B>
where
    B: ByteSlice,
{
    type Target = Header;

    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl<B> DerefMut for Builder<B>
where
    B: ByteSliceMut,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}
