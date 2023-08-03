use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use gaia_stub::tco_tmiv::Tco;

use super::Hook;
use crate::tco_tmiv::tco;

#[derive(Clone)]
pub struct SanitizeHook {
    schema_set: Arc<tco::SchemaSet>,
}

impl SanitizeHook {
    pub fn new(schema_set: impl Into<Arc<tco::SchemaSet>>) -> Self {
        Self {
            schema_set: schema_set.into(),
        }
    }

    fn sanitize(&self, input: &Tco) -> Result<Tco> {
        let sanitized = self
            .schema_set
            .sanitize(input)
            .map_err(|msg| anyhow!("TCO validation error: {}", msg))?;
        Ok(sanitized)
    }
}

#[async_trait]
impl Hook<Arc<Tco>> for SanitizeHook {
    type Output = Arc<Tco>;

    async fn hook(&mut self, tco: Arc<Tco>) -> Result<Self::Output> {
        let sanitized = self.sanitize(&tco)?;
        Ok(Arc::new(sanitized))
    }
}
