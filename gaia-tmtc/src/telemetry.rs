use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::{anyhow, ensure, Result};
use async_trait::async_trait;
use gaia_stub::tco_tmiv::Tmiv;
use tokio::sync::{broadcast, RwLock};

use super::{Handle, Hook};
use crate::tco_tmiv::tmiv;

#[derive(Clone)]
pub struct Bus {
    tmiv_tx: broadcast::Sender<Arc<Tmiv>>,
}

impl Bus {
    pub fn new(capacity: usize) -> Self {
        let (tmiv_tx, _) = broadcast::channel(capacity);
        Self { tmiv_tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Tmiv>> {
        self.tmiv_tx.subscribe()
    }
}

#[async_trait]
impl Handle<Arc<Tmiv>> for Bus {
    type Response = ();

    async fn handle(&mut self, tmiv: Arc<Tmiv>) -> Result<Self::Response> {
        // it's ok if there are no receivers
        // so just fire and forget
        self.tmiv_tx.send(tmiv).ok();
        Ok(())
    }
}

#[derive(Clone)]
pub struct SanitizeHook {
    schema_set: Arc<tmiv::SchemaSet>,
}

impl SanitizeHook {
    pub fn new(schema_set: impl Into<Arc<tmiv::SchemaSet>>) -> Self {
        Self {
            schema_set: schema_set.into(),
        }
    }

    fn sanitize(&self, input: &Tmiv) -> Result<Tmiv> {
        let sanitized = self
            .schema_set
            .sanitize(input)
            .map_err(|msg| anyhow!("TMIV validation error: {}", msg))?;
        Ok(sanitized)
    }
}

#[async_trait]
impl Hook<Arc<Tmiv>> for SanitizeHook {
    type Output = Arc<Tmiv>;

    async fn hook(&mut self, tmiv: Arc<Tmiv>) -> Result<Self::Output> {
        let sanitized = self.sanitize(&tmiv)?;
        Ok(Arc::new(sanitized))
    }
}

pub trait CheckTmivName {
    fn check_tmiv_name(&self, tmiv_name: &str) -> bool;
}

impl CheckTmivName for tmiv::SchemaSet {
    fn check_tmiv_name(&self, tmiv_name: &str) -> bool {
        self.find_schema_by_name(tmiv_name).is_some()
    }
}

impl CheckTmivName for HashSet<String> {
    fn check_tmiv_name(&self, tmiv_name: &str) -> bool {
        self.contains(tmiv_name)
    }
}

pub struct LastTmivStore {
    check_tmiv_name: Box<dyn CheckTmivName + Send + Sync + 'static>,
    map: RwLock<HashMap<String, Arc<Tmiv>>>,
}

impl LastTmivStore {
    pub fn new(check_tmiv_name: impl CheckTmivName + Send + Sync + 'static) -> Self {
        let check_tmiv_name = Box::new(check_tmiv_name);
        Self {
            check_tmiv_name,
            map: RwLock::new(HashMap::new()),
        }
    }

    fn is_valid(&self, telemetry_name: &str) -> bool {
        self.check_tmiv_name.check_tmiv_name(telemetry_name)
    }

    pub async fn set(&self, tmiv: Arc<Tmiv>) {
        let mut map = self.map.write().await;
        map.insert(tmiv.name.clone(), tmiv);
    }

    pub async fn get(&self, telemetry_name: &str) -> Result<Option<Arc<Tmiv>>> {
        ensure!(
            self.is_valid(telemetry_name),
            "no such telemetry definition: {}",
            telemetry_name
        );
        let map = self.map.read().await;
        if let Some(tmiv) = map.get(telemetry_name) {
            Ok(Some(tmiv.clone()))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone)]
pub struct StoreLastTmivHook {
    store: Arc<LastTmivStore>,
}

impl StoreLastTmivHook {
    pub fn new(store: Arc<LastTmivStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Hook<Arc<Tmiv>> for StoreLastTmivHook {
    type Output = Arc<Tmiv>;

    async fn hook(&mut self, tmiv: Arc<Tmiv>) -> Result<Self::Output> {
        self.store.set(tmiv.clone()).await;
        Ok(tmiv)
    }
}
