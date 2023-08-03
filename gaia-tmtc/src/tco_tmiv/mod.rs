use serde::{Deserialize, Serialize};

pub mod tco;
pub mod tmiv;

pub use gaia_stub::tco_tmiv::{Tco, Tmiv, *};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TcoTmivSchema {
    pub tco: Vec<tco::Schema>,
    pub tmiv: Vec<tmiv::Schema>,
}

impl TcoTmivSchema {
    pub fn new(tco: Vec<tco::Schema>, tmiv: Vec<tmiv::Schema>) -> Self {
        Self { tco, tmiv }
    }
}
