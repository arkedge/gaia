use anyhow::Result;

#[async_trait::async_trait]
pub trait SyncAndChannelCoding {
    async fn transmit(
        &mut self,
        scid: u16,
        vcid: u8,
        frame_type: FrameType,
        sequence_number: u8,
        data_field: &[u8],
    ) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameType {
    TypeAD,
    TypeBD,
    TypeBC,
}

impl FrameType {
    pub fn bypass_flag(&self) -> bool {
        match self {
            FrameType::TypeAD => false,
            FrameType::TypeBD => true,
            FrameType::TypeBC => true,
        }
    }

    pub fn control_command_flag(&self) -> bool {
        match self {
            FrameType::TypeAD => false,
            FrameType::TypeBD => false,
            FrameType::TypeBC => true,
        }
    }
}
