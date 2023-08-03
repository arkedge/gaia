use crate::ccsds::{self, tc::clcw::CLCW};

pub type TransferFrame<B> = ccsds::aos::TransferFrame<B, CLCW>;
