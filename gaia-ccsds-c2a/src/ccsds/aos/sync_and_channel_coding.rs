use anyhow::Result;
use zerocopy::Unaligned;

use super::TransferFrame;

#[async_trait::async_trait]
pub trait SyncAndChannelCoding {
    async fn receive(&mut self) -> Result<TransferFrameBuffer>;
}

pub struct TransferFrameBuffer {
    bytes: Vec<u8>,
}

impl TransferFrameBuffer {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn transfer_frame<T: Unaligned>(&self) -> Option<TransferFrame<&[u8], T>> {
        TransferFrame::<_, T>::new(self.bytes.as_slice())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}
