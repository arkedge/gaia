//! gRPC service implementation for telemetry and command recording

use std::sync::Arc;

use anyhow::Result;
use duckdb::Connection;
use gaia_stub::recorder::recorder_server::Recorder;
use gaia_stub::recorder::{PostCommandRequest, PostCommandResponse};
use gaia_stub::recorder::{PostTelemetryRequest, PostTelemetryResponse};
use gaia_stub::tco_tmiv::{Tco, Tmiv};
use prost_types::Timestamp;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::error;

use crate::api::http::RecorderState;
use crate::db;

#[derive(Clone)]
pub struct RecorderService {
    state: Arc<RwLock<RecorderState>>,
}

impl RecorderService {
    pub fn new(state: Arc<RwLock<RecorderState>>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl Recorder for RecorderService {
    async fn post_command(
        &self,
        request: Request<PostCommandRequest>,
    ) -> Result<Response<PostCommandResponse>, Status> {
        let message = request.into_inner();
        let Some(tco) = message.tco else {
            return Err(Status::invalid_argument("tco is required"));
        };
        if let Err(e) = insert_command(&self.state, &tco, message.timestamp).await {
            error!("failed to store command: {e}");
        }
        Ok(Response::new(PostCommandResponse {}))
    }

    async fn post_telemetry(
        &self,
        request: Request<PostTelemetryRequest>,
    ) -> Result<Response<PostTelemetryResponse>, Status> {
        let message = request.into_inner();
        let Some(tmiv) = message.tmiv else {
            return Err(Status::invalid_argument("tmiv is required"));
        };
        if let Err(e) = insert_tmiv(&self.state, &tmiv).await {
            error!("failed to store telemetry: {e}");
        }
        Ok(Response::new(PostTelemetryResponse {}))
    }
}

async fn insert_command(
    state: &Arc<RwLock<RecorderState>>,
    tco: &Tco,
    timestamp: Option<Timestamp>,
) -> Result<()> {
    let session = {
        let guard = state.read().await;
        guard.session.clone()
    };
    let Some(session) = session else {
        return Ok(());
    };

    let time_ms = timestamp
        .map(|ts| ts.seconds * 1000 + (ts.nanos as i64 / 1_000_000))
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let params_json = db::build_params_json(tco);
    let command_name = tco.name.clone();

    tokio::task::spawn_blocking(move || {
        let conn = Connection::open(session.db_path)?;
        db::insert_command_log(&conn, time_ms, &command_name, &params_json)?;
        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}

async fn insert_tmiv(state: &Arc<RwLock<RecorderState>>, tmiv: &Tmiv) -> Result<()> {
    let session = {
        let guard = state.read().await;
        guard.session.clone()
    };
    let Some(session) = session else {
        return Ok(());
    };

    let time_received_ms = tmiv.plugin_received_time as i64 * 1000;
    let time_primary_ms = tmiv
        .timestamp
        .as_ref()
        .map(|ts| ts.seconds * 1000 + (ts.nanos as i64 / 1_000_000))
        .unwrap_or(time_received_ms);

    let tmiv_name = tmiv.name.clone();
    let fields = tmiv.fields.clone();

    tokio::task::spawn_blocking(move || {
        let mut conn = Connection::open(session.db_path)?;
        let tx = conn.transaction()?;
        for field in fields.iter() {
            db::insert_telemetry_sample(
                &tx,
                &tmiv_name,
                field,
                time_primary_ms,
                time_received_ms,
            )?;
        }
        tx.commit()?;
        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}
