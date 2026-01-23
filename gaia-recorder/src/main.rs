mod api;
mod db;
mod domain;
mod transform;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{error_handling::HandleErrorLayer, response::IntoResponse};
use clap::Parser;
use gaia_stub::recorder::recorder_server::RecorderServer;
use tokio::sync::RwLock;
use tonic_web::GrpcWebLayer;
use tower::{BoxError, ServiceBuilder};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

use api::{RecorderService, RecorderState};

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

    // Start initial recording session if not in playback mode
    if !args.playback_mode {
        // Note: start_new_session is now private in api::http module
        // For now, we'll let the first POST to /api/recording/start create the session
        info!("recorder ready to start recording");
    } else {
        info!("playback mode: no new recording will be created");
    }

    let recorder_service = RecorderService::new(state.clone());
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

    let api = api::create_router(state)
        .layer(CorsLayer::permissive())
        .fallback_service(grpc_service);

    let socket_addr = SocketAddr::new(args.bind_addr, args.bind_port);
    info!(message = "starting recorder", %socket_addr);
    axum::Server::bind(&socket_addr)
        .serve(api.into_make_service())
        .await?;

    Ok(())
}