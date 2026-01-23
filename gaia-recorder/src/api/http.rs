//! HTTP API handlers for telemetry recording and querying

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use axum::extract::Query;
use axum::response::IntoResponse;
use axum::{routing::get, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::db;
use crate::domain::{RecordingInfo, Session, SessionInfo, SessionState};

#[derive(Clone, Debug)]
pub struct RecorderState {
    pub data_dir: PathBuf,
    pub session: Option<SessionInfo>,
    pub playback_mode: bool,
    pub schema_json: Option<String>,
}

impl RecorderState {
    pub fn new(data_dir: PathBuf, playback_mode: bool, schema_json: Option<String>) -> Self {
        Self {
            data_dir,
            session: None,
            playback_mode,
            schema_json,
        }
    }
}

impl SessionState for RecorderState {
    fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn current_session(&self) -> Option<SessionInfo> {
        self.session.clone()
    }
}

#[derive(Debug, Deserialize)]
struct StartRecordingRequest {
    suffix: Option<String>,
}

#[derive(Debug, Serialize)]
struct StartRecordingResponse {
    session: SessionInfo,
}

#[derive(Debug, Serialize)]
struct StopRecordingResponse {
    session: Option<SessionInfo>,
}

#[derive(Debug, Serialize)]
struct CurrentSessionResponse {
    session: Option<SessionInfo>,
}

#[derive(Debug, Deserialize)]
struct TelemetryQuery {
    session_id: Option<String>,
    tmiv_name: String,
    field_name: String,
    is_raw: bool,
    start_ms: i64,
    end_ms: i64,
    max_points: Option<usize>,
    time_axis: Option<String>,
}

#[derive(Debug, Serialize)]
struct TelemetryQueryResponse {
    samples: Vec<db::TelemetrySample>,
}

#[derive(Debug, Deserialize)]
struct CommandQuery {
    session_id: Option<String>,
    start_ms: i64,
    end_ms: i64,
    max_points: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct TimeRangeQuery {
    session_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct TimeRangeResponse {
    min_time_ms: Option<i64>,
    max_time_ms: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CommandQueryResponse {
    commands: Vec<db::CommandLogItem>,
}

#[derive(Debug, Serialize)]
struct RecordingListResponse {
    recordings: Vec<RecordingInfo>,
}

pub fn create_router(state: Arc<RwLock<RecorderState>>) -> Router {
    Router::new()
        .route("/api/recording/start", post(start_recording))
        .route("/api/recording/stop", post(stop_recording))
        .route("/api/recording/session", get(current_session))
        .route("/api/recordings/list", get(list_recordings))
        .route("/api/telemetry/query", get(query_telemetry))
        .route("/api/telemetry/time_range", get(get_time_range))
        .route("/api/commands/query", get(query_commands))
        .route("/api/schema", get(get_schema))
        .with_state(state)
}

async fn start_recording(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
    Json(body): Json<StartRecordingRequest>,
) -> Result<Json<StartRecordingResponse>, axum::http::StatusCode> {
    // Check if playback mode is enabled
    let data_dir = {
        let guard = state.read().await;
        if guard.playback_mode {
            return Err(axum::http::StatusCode::FORBIDDEN);
        }
        guard.data_dir.clone()
    };

    let session = Session::create(&data_dir, body.suffix)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Store session in state
    {
        let mut guard = state.write().await;
        guard.session = Some(session.clone());
    }

    Ok(Json(StartRecordingResponse { session }))
}

async fn stop_recording(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
) -> Result<Json<StopRecordingResponse>, axum::http::StatusCode> {
    // Check if playback mode is enabled
    {
        let guard = state.read().await;
        if guard.playback_mode {
            return Err(axum::http::StatusCode::FORBIDDEN);
        }
    }
    let mut guard = state.write().await;
    let session = guard.session.take();
    Ok(Json(StopRecordingResponse { session }))
}

async fn current_session(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
) -> Result<Json<CurrentSessionResponse>, axum::http::StatusCode> {
    let guard = state.read().await;
    Ok(Json(CurrentSessionResponse {
        session: guard.session.clone(),
    }))
}

async fn query_telemetry(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
    Query(query): Query<TelemetryQuery>,
) -> Result<Json<TelemetryQueryResponse>, axum::http::StatusCode> {
    // Log the query for debugging
    tracing::debug!(
        "Telemetry query: session_id={:?}, tmiv={}, field={}, is_raw={}, start={}, end={}, max_points={:?}",
        query.session_id, query.tmiv_name, query.field_name, query.is_raw, query.start_ms, query.end_ms, query.max_points
    );

    let db_path = crate::domain::session::resolve_session_path_from_state(&state, query.session_id.clone())
        .await
        .unwrap_or_default();
    if db_path.is_empty() {
        tracing::warn!("No database path found for session_id={:?}", query.session_id);
        return Ok(Json(TelemetryQueryResponse { samples: vec![] }));
    }

    // In playback mode, adjust time range to match actual data
    let (start_ms, end_ms) = {
        let guard = state.read().await;
        if guard.playback_mode {
            let db_path_clone = db_path.clone();
            let (db_start, db_end) = tokio::task::spawn_blocking(move || {
                db::query_time_range(&db_path_clone)
            })
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
            .and_then(|res| res.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR))?;

            if let (Some(db_start), Some(db_end)) = (db_start, db_end) {
                tracing::info!(
                    "Playback mode: adjusting query time range from [{}, {}] to database range [{}, {}]",
                    query.start_ms, query.end_ms, db_start, db_end
                );
                (db_start, db_end)
            } else {
                (query.start_ms, query.end_ms)
            }
        } else {
            (query.start_ms, query.end_ms)
        }
    };

    let time_axis = query.time_axis.unwrap_or_else(|| "primary".to_string());
    let tmiv_name = query.tmiv_name.clone();
    let field_name = query.field_name.clone();
    let is_raw = query.is_raw;
    let samples = tokio::task::spawn_blocking(move || {
        db::query_telemetry(
            &db_path,
            &tmiv_name,
            &field_name,
            is_raw,
            start_ms,
            end_ms,
            query.max_points.unwrap_or(2000),
            &time_axis,
        )
    })
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
    .and_then(|res| res.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR))?;

    tracing::debug!("Telemetry query returned {} samples", samples.len());
    Ok(Json(TelemetryQueryResponse { samples }))
}

async fn query_commands(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
    Query(query): Query<CommandQuery>,
) -> Result<Json<CommandQueryResponse>, axum::http::StatusCode> {
    let db_path = crate::domain::session::resolve_session_path_from_state(&state, query.session_id.clone())
        .await
        .unwrap_or_default();
    if db_path.is_empty() {
        return Ok(Json(CommandQueryResponse { commands: vec![] }));
    }
    let commands = tokio::task::spawn_blocking(move || {
        db::query_commands(
            &db_path,
            query.start_ms,
            query.end_ms,
            query.max_points.unwrap_or(10000), // Increased default limit
        )
    })
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
    .and_then(|res| res.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok(Json(CommandQueryResponse { commands }))
}

async fn get_time_range(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
    Query(query): Query<TimeRangeQuery>,
) -> Result<Json<TimeRangeResponse>, axum::http::StatusCode> {
    let db_path = crate::domain::session::resolve_session_path_from_state(&state, query.session_id.clone())
        .await
        .unwrap_or_default();
    if db_path.is_empty() {
        return Ok(Json(TimeRangeResponse {
            min_time_ms: None,
            max_time_ms: None,
        }));
    }
    let (min_time_ms, max_time_ms) = tokio::task::spawn_blocking(move || {
        db::query_time_range(&db_path)
    })
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
    .and_then(|res| res.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok(Json(TimeRangeResponse {
        min_time_ms,
        max_time_ms,
    }))
}

async fn get_schema(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    let guard = state.read().await;
    match &guard.schema_json {
        Some(json) => Ok((
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            json.clone(),
        )
            .into_response()),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

async fn list_recordings(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
) -> Result<Json<RecordingListResponse>, axum::http::StatusCode> {
    let data_dir = {
        let guard = state.read().await;
        guard.data_dir.clone()
    };
    let recordings = tokio::task::spawn_blocking(move || Session::list_all(&data_dir))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        .and_then(|res| res.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(RecordingListResponse { recordings }))
}

