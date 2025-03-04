use std::collections::HashMap;

use crate::ccsds::aos::{m_pdu::Defragmenter, virtual_channel::Synchronizer};

#[derive(Debug, Default)]
pub struct VirtualChannel {
    pub synchronizer: Synchronizer,
    pub defragmenter: Defragmenter,
}

#[derive(Debug, Default)]
pub struct Demuxer {
    channels: HashMap<u8, VirtualChannel>,
}

impl Demuxer {
    pub fn demux(&mut self, vcid: u8) -> &mut VirtualChannel {
        self.channels.entry(vcid).or_default()
    }
}
