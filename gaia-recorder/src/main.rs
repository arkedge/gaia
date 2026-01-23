mod api;
mod db;
mod domain;
mod frontend_server;
mod transform;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use gaia_stub::recorder::recorder_server::RecorderServer;
use tokio::sync::RwLock;
use tonic::transport::Server;
use tonic_web::GrpcWebLayer;
use tower::ServiceBuilder;
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
    /// Path to tlmcmddb JSON file (telemetry/command schema)
    #[clap(long, env)]
    tlmcmddb: Option<PathBuf>,
    /// Path to satconfig JSON file (satellite configuration, deprecated - use tlmcmddb)
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

    // Load schema JSON if provided (prefer tlmcmddb over schema_file)
    let schema_json = if let Some(tlmcmddb_path) = &args.tlmcmddb {
        match std::fs::read_to_string(tlmcmddb_path) {
            Ok(content) => {
                info!("loaded tlmcmddb from {:?}", tlmcmddb_path);
                Some(content)
            }
            Err(e) => {
                error!("failed to read tlmcmddb file {:?}: {}", tlmcmddb_path, e);
                None
            }
        }
    } else if let Some(schema_path) = &args.schema_file {
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
        args.data_dir.clone(),
        args.playback_mode,
        schema_json,
    )));

    // Start automatic temporary session if not in playback mode
    if args.playback_mode {
        info!("playback mode: no new recording will be created");
    } else {
        match domain::Session::create(&args.data_dir, Some("_auto".to_string())).await {
            Ok(session_info) => {
                let mut guard = state.write().await;
                guard.session = Some(session_info.clone());
                info!("started temporary recording session: {} at {}", session_info.session_id, session_info.db_path);
                info!("this temporary session will be deleted on exit unless you click the PLAY button to save it");
            }
            Err(e) => {
                error!("failed to create temporary session: {}", e);
            }
        }
    }

    // Enable gRPC service (follow tmtc-c2a pattern: include CORS in gRPC layer)
    let recorder_service = RecorderService::new(state.clone());
    let grpc_web_layer = GrpcWebLayer::new();
    let cors_layer = CorsLayer::new()
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any)
        .allow_origin(tower_http::cors::Any);
    let trace_layer = TraceLayer::new_for_grpc();
    let layer = ServiceBuilder::new()
        .layer(trace_layer)
        .layer(cors_layer)
        .layer(grpc_web_layer);

    let grpc_service = Server::builder()
        .layer(layer)
        .add_service(RecorderServer::new(recorder_service))
        .into_service();

    // Follow tmtc-c2a pattern exactly:
    // - Nest frontend at specific path
    // - Redirect root to frontend
    // - Use fallback_service with HandleError for gRPC
    use axum::routing::get;
    use axum::response::Redirect;
    use axum::error_handling::HandleError;

    async fn handle_grpc_error(
        err: Box<dyn std::error::Error + Send + Sync>,
    ) -> impl axum::response::IntoResponse {
        error!("grpc service error: {err}");
        (
            axum::http::StatusCode::OK,
            [
                ("content-type", "application/grpc".to_owned()),
                ("grpc-status", "13".to_owned()),
                ("content-type", format!("internal error: {err}")),
            ],
        )
    }

    // Create API router
    let api_router = api::http::create_router(state.clone());

    // Exactly copy tmtc-c2a pattern
    let app = axum::Router::new()
        .merge(api_router)
        .nest(
            "/devtools/",
            axum::Router::new().fallback(frontend_server::serve),
        )
        .route("/", get(|| async { Redirect::to("/devtools/") }))
        .route("/devtools", get(|| async { Redirect::to("/devtools/") }))
        .fallback_service(HandleError::new(grpc_service, handle_grpc_error));

    let socket_addr = SocketAddr::new(args.bind_addr, args.bind_port);
    info!(message = "starting recorder", %socket_addr);

    // Graceful shutdown signal handler
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C signal handler");
    };

    let server = axum::Server::bind(&socket_addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal);

    server.await?;

    // Cleanup: Remove temporary databases (_auto suffix)
    if !args.playback_mode {
        cleanup_temp_databases(&args.data_dir).await;
    }

    Ok(())
}

async fn cleanup_temp_databases(data_dir: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with("_auto.duckdb") {
                    if let Err(e) = std::fs::remove_file(&path) {
                        error!("failed to remove temporary database {}: {}", filename, e);
                    }
                }
            }
        }
    }
}
