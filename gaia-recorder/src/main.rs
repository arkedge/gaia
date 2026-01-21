use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    error_handling::HandleErrorLayer,
    extract::Query,
    response::IntoResponse,
    routing::get,
    routing::post,
    Json,
    Router,
};
use clap::Parser;
use gaia_stub::recorder::recorder_server::{Recorder, RecorderServer};
use gaia_stub::recorder::{PostCommandRequest, PostCommandResponse, PostTelemetryRequest, PostTelemetryResponse};
use gaia_stub::tco_tmiv::{self, tco_param, Tco, Tmiv, TmivField};
use prost_types::Timestamp;
use duckdb::{params, Connection};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use tower::{BoxError, ServiceBuilder};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, env, default_value_t = Ipv4Addr::UNSPECIFIED.into())]
    bind_addr: IpAddr,
    #[clap(long, env, default_value_t = 8920)]
    bind_port: u16,
    #[clap(long, env, default_value = "recordings")]
    data_dir: PathBuf,
    /// Enable playback mode (read-only, no new recording)
    #[clap(long, env)]
    playback_mode: bool,
    /// Path to satellite schema JSON file (optional, for standalone viewer)
    #[clap(long, env)]
    schema_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
struct SessionInfo {
    session_id: String,
    suffix: String,
    started_at_ms: i64,
    db_path: String,
    active: bool,
}

#[derive(Clone, Debug)]
struct RecorderState {
    data_dir: PathBuf,
    session: Option<SessionInfo>,
    playback_mode: bool,
    schema_json: Option<String>,
}

impl RecorderState {
    fn new(data_dir: PathBuf, playback_mode: bool, schema_json: Option<String>) -> Self {
        Self {
            data_dir,
            session: None,
            playback_mode,
            schema_json,
        }
    }
}

#[derive(Clone)]
struct RecorderSvc {
    state: Arc<RwLock<RecorderState>>,
}

#[tonic::async_trait]
impl Recorder for RecorderSvc {
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
struct TelemetrySample {
    time_ms: i64,
    value_type: String,
    value_num: Option<f64>,
    value_int: Option<i64>,
    value_text: Option<String>,
    value_bytes_hex: Option<String>,
}

#[derive(Debug, Serialize)]
struct TelemetryQueryResponse {
    samples: Vec<TelemetrySample>,
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
struct CommandLogItem {
    time_ms: i64,
    command_name: String,
    params_json: String,
}

#[derive(Debug, Serialize)]
struct CommandQueryResponse {
    commands: Vec<CommandLogItem>,
}

#[derive(Debug, Serialize)]
struct RecordingListItem {
    session_id: String,
    suffix: String,
    started_at_ms: Option<i64>,
    db_path: String,
}

#[derive(Debug, Serialize)]
struct RecordingListResponse {
    recordings: Vec<RecordingListItem>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(false))
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    std::fs::create_dir_all(&args.data_dir)
        .with_context(|| format!("failed to create data dir {:?}", args.data_dir))?;

    // Load schema JSON if provided
    let schema_json = if let Some(schema_path) = &args.schema_file {
        match std::fs::read_to_string(schema_path) {
            Ok(content) => {
                info!("loaded schema from {:?}", schema_path);
                Some(content)
            }
            Err(e) => {
                error!("failed to read schema file {:?}: {}", schema_path, e);
                None
            }
        }
    } else {
        None
    };

    let state = Arc::new(RwLock::new(RecorderState::new(
        args.data_dir,
        args.playback_mode,
        schema_json,
    )));

    if !args.playback_mode {
        let session = start_new_session(&state, None).await?;
        info!("recording started: {}", session.db_path);
    } else {
        info!("playback mode: no new recording will be created");
    }

    let recorder_service = RecorderSvc { state: state.clone() };
    let grpc_web_layer = GrpcWebLayer::new();
    let trace_layer = TraceLayer::new_for_grpc();
    let layer = ServiceBuilder::new()
        .layer(trace_layer)
        .layer(grpc_web_layer);

    let grpc_service = tonic::transport::Server::builder()
        .layer(layer)
        .add_service(RecorderServer::new(recorder_service))
        .into_service();
    let grpc_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|err: BoxError| async move {
            error!("grpc service error: {err}");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
        .service(grpc_service);

    let api = Router::new()
        .route("/api/recording/start", post(start_recording))
        .route("/api/recording/stop", post(stop_recording))
        .route("/api/recording/session", get(current_session))
        .route("/api/recordings/list", get(list_recordings))
        .route("/api/telemetry/query", get(query_telemetry))
        .route("/api/telemetry/time_range", get(get_time_range))
        .route("/api/commands/query", get(query_commands))
        .route("/api/schema", get(get_schema))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .fallback_service(grpc_service);

    let socket_addr = SocketAddr::new(args.bind_addr, args.bind_port);
    info!(message = "starting recorder", %socket_addr);
    axum::Server::bind(&socket_addr)
        .serve(api.into_make_service())
        .await?;

    Ok(())
}

async fn start_recording(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RecorderState>>>,
    Json(body): Json<StartRecordingRequest>,
) -> Result<Json<StartRecordingResponse>, axum::http::StatusCode> {
    // Check if playback mode is enabled
    {
        let guard = state.read().await;
        if guard.playback_mode {
            return Err(axum::http::StatusCode::FORBIDDEN);
        }
    }
    let suffix = body.suffix.unwrap_or_default();
    let session = start_new_session(&state, Some(suffix))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
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

    let db_path = resolve_session_path(&state, query.session_id.clone())
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
                get_time_range_from_db(&db_path_clone)
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
        query_telemetry_from_db(
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
    let db_path = resolve_session_path(&state, query.session_id.clone())
        .await
        .unwrap_or_default();
    if db_path.is_empty() {
        return Ok(Json(CommandQueryResponse { commands: vec![] }));
    }
    let commands = tokio::task::spawn_blocking(move || {
        query_commands_from_db(
            &db_path,
            query.start_ms,
            query.end_ms,
            query.max_points.unwrap_or(10000),  // Increased default limit
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
    let db_path = resolve_session_path(&state, query.session_id.clone())
        .await
        .unwrap_or_default();
    if db_path.is_empty() {
        return Ok(Json(TimeRangeResponse {
            min_time_ms: None,
            max_time_ms: None,
        }));
    }
    let (min_time_ms, max_time_ms) = tokio::task::spawn_blocking(move || {
        get_time_range_from_db(&db_path)
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
    let recordings = tokio::task::spawn_blocking(move || list_recording_files(&data_dir))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        .and_then(|res| res.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(RecordingListResponse { recordings }))
}

async fn start_new_session(
    state: &Arc<RwLock<RecorderState>>,
    suffix: Option<String>,
) -> Result<SessionInfo> {
    let suffix = suffix.unwrap_or_default();
    let started_at = chrono::Utc::now();
    let session_id = started_at.format("%Y%m%d_%H%M%S").to_string();
    let file_name = if suffix.is_empty() {
        format!("recording_{session_id}.duckdb")
    } else {
        format!("recording_{session_id}_{suffix}.duckdb")
    };
    let db_path = {
        let guard = state.read().await;
        guard.data_dir.join(file_name)
    };

    let db_path_string = db_path.to_string_lossy().to_string();

    tokio::task::spawn_blocking({
        let db_path = db_path.clone();
        move || init_db(&db_path)
    })
    .await??;

    let session = SessionInfo {
        session_id,
        suffix,
        started_at_ms: started_at.timestamp_millis(),
        db_path: db_path_string,
        active: true,
    };

    let mut guard = state.write().await;
    guard.session = Some(session.clone());

    Ok(session)
}

fn init_db(path: &Path) -> Result<()> {
    let conn = Connection::open(path)?;

    // DuckDB automatically uses compression; optimized data types reduce storage
    conn.execute_batch(
        "
        CREATE SEQUENCE IF NOT EXISTS seq_telemetry_samples START 1;
        CREATE TABLE IF NOT EXISTS telemetry_samples (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_telemetry_samples'),
            tmiv_name VARCHAR NOT NULL,
            field_name VARCHAR NOT NULL,
            is_raw TINYINT NOT NULL,
            time_primary_ms BIGINT NOT NULL,
            time_received_ms BIGINT NOT NULL,
            value_type VARCHAR(20) NOT NULL,
            value_num DOUBLE,
            value_int BIGINT,
            value_text VARCHAR,
            value_bytes BLOB
        );
        CREATE INDEX IF NOT EXISTS idx_telemetry_query
            ON telemetry_samples (tmiv_name, field_name, is_raw, time_primary_ms);

        CREATE SEQUENCE IF NOT EXISTS seq_command_logs START 1;
        CREATE TABLE IF NOT EXISTS command_logs (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_command_logs'),
            time_ms BIGINT NOT NULL,
            command_name VARCHAR NOT NULL,
            params_json VARCHAR NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_command_time ON command_logs (time_ms);
        ",
    )?;
    Ok(())
}

async fn resolve_session_path(
    state: &Arc<RwLock<RecorderState>>,
    session_id: Option<String>,
) -> Option<String> {
    if let Some(session_id) = session_id {
        let data_dir = {
            let guard = state.read().await;
            guard.data_dir.clone()
        };
        let list = tokio::task::spawn_blocking(move || list_recording_files(&data_dir))
            .await
            .ok()
            .and_then(|res| res.ok())?;
        let item = list.into_iter().find(|item| item.session_id == session_id)?;
        return Some(item.db_path);
    }
    let guard = state.read().await;
    guard.session.as_ref().map(|session| session.db_path.clone())
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

    let params_json = build_params_json(tco);
    let command_name = tco.name.clone();

    tokio::task::spawn_blocking(move || {
        let conn = Connection::open(session.db_path)?;
        conn.execute(
            "INSERT INTO command_logs (time_ms, command_name, params_json) VALUES (?1, ?2, ?3)",
            params![time_ms, command_name, params_json],
        )?;
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
            insert_tmiv_field(
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

fn insert_tmiv_field(
    tx: &duckdb::Transaction<'_>,
    tmiv_name: &str,
    field: &TmivField,
    time_primary_ms: i64,
    time_received_ms: i64,
) -> Result<()> {
    // Store field_name with :raw or :conv suffix (same format as CSV import)
    let (field_name, is_raw) = if field.name.ends_with("@RAW") {
        let base_name = field.name.trim_end_matches("@RAW").replace('_', ".");
        (format!("{}:raw", base_name), 1)
    } else {
        let base_name = field.name.replace('_', ".");
        (format!("{}:conv", base_name), 0)
    };

    let mut value_num: Option<f64> = None;
    let mut value_int: Option<i64> = None;
    let mut value_text: Option<String> = None;
    let mut value_bytes: Option<Vec<u8>> = None;
    let value_type: &str;

    match field.value.as_ref() {
        Some(tco_tmiv::tmiv_field::Value::Integer(i)) => {
            value_type = "integer";
            value_int = Some(*i);
            value_num = Some(*i as f64);
        }
        Some(tco_tmiv::tmiv_field::Value::Double(d)) => {
            value_type = "double";
            value_num = Some(*d);
        }
        Some(tco_tmiv::tmiv_field::Value::Enum(e)) => {
            value_type = "enum";
            value_text = Some(e.clone());
        }
        Some(tco_tmiv::tmiv_field::Value::String(s)) => {
            value_type = "string";
            value_text = Some(s.clone());
        }
        Some(tco_tmiv::tmiv_field::Value::Bytes(b)) => {
            value_type = "bytes";
            value_bytes = Some(b.clone());
            if b.len() <= 8 {
                let mut buf = [0u8; 8];
                buf[8 - b.len()..].copy_from_slice(b);
                let raw = u64::from_be_bytes(buf) as i64;
                value_int = Some(raw);
                value_num = Some(raw as f64);
            }
        }
        None => {
            value_type = "unknown";
        }
    }

    tx.execute(
        "INSERT INTO telemetry_samples (tmiv_name, field_name, is_raw, time_primary_ms, time_received_ms, value_type, value_num, value_int, value_text, value_bytes)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            tmiv_name,
            field_name,
            is_raw,
            time_primary_ms,
            time_received_ms,
            value_type,
            value_num,
            value_int,
            value_text,
            value_bytes,
        ],
    )?;

    Ok(())
}

fn build_params_json(tco: &Tco) -> String {
    let params: Vec<serde_json::Value> = tco
        .params
        .iter()
        .map(|param| {
            let value = match param.value.as_ref() {
                Some(tco_param::Value::Integer(v)) => serde_json::json!({"type": "integer", "value": v}),
                Some(tco_param::Value::Double(v)) => serde_json::json!({"type": "double", "value": v}),
                Some(tco_param::Value::Bytes(v)) => {
                    let hex = v.iter().map(|b| format!("{:02x}", b)).collect::<String>();
                    serde_json::json!({"type": "bytes", "value": hex})
                }
                None => serde_json::json!({"type": "none"}),
            };
            serde_json::json!({"name": param.name, "value": value})
        })
        .collect();

    serde_json::json!({"name": tco.name, "params": params}).to_string()
}

fn query_telemetry_from_db(
    db_path: &str,
    tmiv_name: &str,
    field_name: &str,
    _is_raw: bool,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
    time_axis: &str,
) -> Result<Vec<TelemetrySample>> {
    let conn = Connection::open(db_path)?;
    let time_column = if time_axis == "received" {
        "time_received_ms"
    } else {
        "time_primary_ms"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {time_column}, value_type, value_num, value_int, value_text, value_bytes
         FROM telemetry_samples
         WHERE tmiv_name = ?1 AND field_name = ?2 AND {time_column} BETWEEN ?3 AND ?4
         ORDER BY {time_column} ASC"
    ))?;

    let rows = stmt.query_map(
        params![tmiv_name, field_name, start_ms, end_ms],
        |row| {
            let time_ms: i64 = row.get(0)?;
            let value_type: String = row.get(1)?;
            let value_num: Option<f64> = row.get(2)?;
            let value_int: Option<i64> = row.get(3)?;
            let value_text: Option<String> = row.get(4)?;
            let value_bytes: Option<Vec<u8>> = row.get(5)?;
            let value_bytes_hex = value_bytes.as_ref().map(|bytes| {
                bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>()
            });
            Ok(TelemetrySample {
                time_ms,
                value_type,
                value_num,
                value_int,
                value_text,
                value_bytes_hex,
            })
        },
    )?;

    let mut samples: Vec<TelemetrySample> = rows.collect::<std::result::Result<_, _>>()?;
    if samples.len() > max_points && max_points > 0 {
        samples = downsample_samples(samples, max_points);
    }
    Ok(samples)
}

fn downsample_samples(samples: Vec<TelemetrySample>, max_points: usize) -> Vec<TelemetrySample> {
    let has_numeric = samples.iter().any(|sample| {
        sample.value_num.is_some() || sample.value_int.is_some()
    });
    if !has_numeric {
        return downsample_stride(samples, max_points);
    }
    downsample_min_max_avg(samples, max_points)
}

fn downsample_stride(samples: Vec<TelemetrySample>, max_points: usize) -> Vec<TelemetrySample> {
    if samples.len() <= max_points {
        return samples;
    }
    let step = (samples.len() as f64 / max_points as f64).ceil() as usize;
    samples
        .into_iter()
        .step_by(step.max(1))
        .take(max_points)
        .collect()
}

fn downsample_min_max_avg(samples: Vec<TelemetrySample>, max_points: usize) -> Vec<TelemetrySample> {
    if samples.len() <= max_points {
        return samples;
    }
    let buckets = max_points / 3;
    if buckets == 0 {
        return samples;
    }
    let bucket_size = (samples.len() as f64 / buckets as f64).ceil() as usize;
    let mut downsampled = Vec::with_capacity(max_points);

    for chunk in samples.chunks(bucket_size) {
        let mut min: Option<f64> = None;
        let mut max: Option<f64> = None;
        let mut sum = 0.0;
        let mut count = 0.0;
        for sample in chunk.iter() {
            let value = sample.value_num.or(sample.value_int.map(|v| v as f64));
            let Some(value) = value else { continue; };
            sum += value;
            count += 1.0;
            min = Some(min.map_or(value, |m| m.min(value)));
            max = Some(max.map_or(value, |m| m.max(value)));
        }
        if count == 0.0 {
            continue;
        }
        let avg = sum / count;
        let time_ms = chunk[chunk.len() / 2].time_ms;
        let make_sample = |value: f64| TelemetrySample {
            time_ms,
            value_type: "double".to_string(),
            value_num: Some(value),
            value_int: None,
            value_text: None,
            value_bytes_hex: None,
        };
        if let Some(min) = min {
            downsampled.push(make_sample(min));
        }
        if let Some(max) = max {
            downsampled.push(make_sample(max));
        }
        downsampled.push(make_sample(avg));
        if downsampled.len() >= max_points {
            break;
        }
    }

    downsampled.truncate(max_points);
    downsampled
}

fn query_commands_from_db(
    db_path: &str,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> Result<Vec<CommandLogItem>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT time_ms, command_name, params_json
         FROM command_logs
         WHERE time_ms BETWEEN ?1 AND ?2
         ORDER BY time_ms ASC
         LIMIT ?3",
    )?;

    let rows = stmt.query_map(params![start_ms, end_ms, max_points as i64], |row| {
        Ok(CommandLogItem {
            time_ms: row.get(0)?,
            command_name: row.get(1)?,
            params_json: row.get(2)?,
        })
    })?;

    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn get_time_range_from_db(db_path: &str) -> Result<(Option<i64>, Option<i64>)> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT MIN(time_primary_ms), MAX(time_primary_ms) FROM telemetry_samples",
    )?;

    let result = stmt.query_row([], |row| {
        Ok((row.get(0).ok(), row.get(1).ok()))
    })?;

    Ok(result)
}

fn list_recording_files(data_dir: &Path) -> Result<Vec<RecordingListItem>> {
    let mut recordings = Vec::new();
    for entry in std::fs::read_dir(data_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !file_name.starts_with("recording_") || !file_name.ends_with(".duckdb") {
            continue;
        }
        let trimmed = file_name
            .trim_start_matches("recording_")
            .trim_end_matches(".duckdb");
        // Split into at most 3 parts: YYYYMMDD, HHMMSS, and optional suffix
        let parts: Vec<&str> = trimmed.splitn(3, '_').collect();
        let (session_id, suffix) = if parts.len() >= 2 {
            // We have at least YYYYMMDD_HHMMSS
            let session_id = format!("{}_{}", parts[0], parts[1]);
            let suffix = if parts.len() >= 3 {
                parts[2].to_string()
            } else {
                String::new()
            };
            (session_id, suffix)
        } else {
            // Fallback for unexpected format
            (trimmed.to_string(), String::new())
        };
        let started_at_ms = chrono::NaiveDateTime::parse_from_str(&session_id, "%Y%m%d_%H%M%S")
            .ok()
            .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).timestamp_millis());
        recordings.push(RecordingListItem {
            session_id,
            suffix,
            started_at_ms,
            db_path: path.to_string_lossy().to_string(),
        });
    }
    recordings.sort_by(|a, b| b.session_id.cmp(&a.session_id));
    Ok(recordings)
}
