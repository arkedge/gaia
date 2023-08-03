#![allow(clippy::identity_op)]

use modular_bitfield_msb::prelude::*;
use zerocopy::{AsBytes, FromBytes, Unaligned};

#[bitfield(bytes = 4)]
#[derive(Debug, Default, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct CLCW {
    pub control_word_type: B1,
    pub clcw_version_number: B2,
    pub status_field: B3,
    pub cop_in_effect: B2,
    pub virtual_channel_identification: B6,
    #[skip]
    __: B2,
    pub no_rf_available: B1,
    pub no_bit_lock: B1,
    pub lockout: B1,
    pub wait: B1,
    pub retransmit: B1,
    pub farm_b_counter: B2,
    #[skip]
    __: B1,
    pub report_value: B8,
}
