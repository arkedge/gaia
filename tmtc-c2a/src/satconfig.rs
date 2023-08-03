use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Satconfig {
    pub aos_scid: u8,
    pub tc_scid: u16,
    pub tlm_apid_map: HashMap<u16, String>,
    pub cmd_apid_map: HashMap<String, u16>,
    pub tlm_channel_map: TelemetryChannelMap,
    pub cmd_prefix_map: CommandPrefixMap,
}

pub type TelemetryChannelMap = HashMap<String, TelemetryChannel>;

#[derive(Debug, Clone, Deserialize)]
pub struct TelemetryChannel {
    pub destination_flag_mask: u8,
}

pub type CommandPrefixMap = HashMap<String, HashMap<String, CommandSubsystem>>;

#[derive(Debug, Clone, Deserialize)]
pub struct CommandSubsystem {
    pub has_time_indicator: bool,
    pub destination_type: u8,
    pub execution_type: u8,
}
