pub mod broker {
    tonic::include_proto!("broker");

    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("broker_descriptor");
}

pub mod recorder {
    tonic::include_proto!("recorder");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("recorder_descriptor");
}

pub mod tco_tmiv {
    tonic::include_proto!("tco_tmiv");

    pub mod tmiv {
        pub fn get_timestamp(tmiv: &super::Tmiv, pseudo_nanos: i32) -> prost_types::Timestamp {
            tmiv.timestamp.clone().unwrap_or(prost_types::Timestamp {
                seconds: tmiv.plugin_received_time as i64,
                nanos: pseudo_nanos,
            })
        }
    }
}
