use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs, io};

use anyhow::{Context, Result};
use clap::Parser;
use gaia_tmtc::broker::broker_server::BrokerServer;
use gaia_tmtc::{
    broker::{self, BrokerService},
    handler,
    telemetry::{self, LastTmivStore},
};
use tmtc_c2a::proto::tmtc_generic_c2a::tmtc_generic_c2a_server::TmtcGenericC2aServer;
use tonic::transport::{NamedService, Server};
use tonic_health::server::HealthReporter;
use tonic_web::GrpcWebLayer;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

use tmtc_c2a::{kble_gs, proto, registry, satellite, Satconfig};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(long, env, default_value_t = Ipv4Addr::UNSPECIFIED.into())]
    broker_addr: IpAddr,
    #[clap(long, env, default_value_t = 8900)]
    broker_port: u16,
    #[clap(long, env, default_value_t = Ipv4Addr::UNSPECIFIED.into())]
    kble_addr: IpAddr,
    #[clap(long, env, default_value_t = 8910)]
    kble_port: u16,
    #[clap(long, env, default_value_t = 1.0)]
    traces_sample_rate: f32,
    #[clap(long, env)]
    sentry_dsn: Option<sentry::types::Dsn>,
    #[clap(env, long)]
    tlmcmddb: PathBuf,
    #[clap(env, long)]
    satconfig: PathBuf,
}

impl Args {
    fn load_satconfig(&self) -> Result<Satconfig> {
        let file = fs::OpenOptions::new().read(true).open(&self.satconfig)?;
        Ok(serde_json::from_reader(&file)?)
    }

    fn load_tlmcmddb(&self) -> Result<tlmcmddb::Database> {
        let file = fs::OpenOptions::new().read(true).open(&self.tlmcmddb)?;
        let rdr = io::BufReader::new(file);
        Ok(serde_json::from_reader(rdr)?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let _guard = sentry::init(sentry::ClientOptions {
        dsn: args.sentry_dsn.clone(),
        traces_sample_rate: args.traces_sample_rate,
        release: sentry::release_name!(),
        ..sentry::ClientOptions::default()
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(false))
        .with(sentry_tracing::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let satconfig = args.load_satconfig().context("Loading satconf")?;
    let tlmcmddb = args.load_tlmcmddb().context("Loading tlmcmddb")?;
    let tlm_registry = registry::TelemetryRegistry::from_tlmcmddb_with_apid_map(
        &tlmcmddb,
        &satconfig.tlm_apid_map,
        satconfig.tlm_channel_map,
    )?;
    let cmd_registry = registry::CommandRegistry::from_tlmcmddb_with_satconfig(
        &tlmcmddb,
        &satconfig.cmd_apid_map,
        satconfig.cmd_prefix_map,
    )?;
    let tmtc_generic_c2a_service =
        proto::tmtc_generic_c2a::Service::new(&tlm_registry, &cmd_registry)?;

    let tlm_bus = telemetry::Bus::new(20);

    let all_tmiv_names = tlm_registry.all_tmiv_names();
    let last_tmiv_store = Arc::new(LastTmivStore::new(all_tmiv_names));
    let store_last_tmiv_hook = telemetry::StoreLastTmivHook::new(last_tmiv_store.clone());
    let tlm_handler: gaia_tmtc::BeforeHook<telemetry::Bus, telemetry::StoreLastTmivHook> =
        handler::Builder::new()
            .before_hook(store_last_tmiv_hook)
            .build(tlm_bus.clone());

    let (link, socket) = kble_gs::new();
    let kble_socket_fut = socket.serve((args.kble_addr, args.kble_port));

    let (satellite_svc, sat_tlm_reporter) = satellite::new(
        satconfig.aos_scid,
        satconfig.tc_scid,
        tlm_registry,
        cmd_registry,
        link.downlink(),
        link.uplink(),
    );
    let sat_tlm_reporter_task = sat_tlm_reporter.run(tlm_handler.clone());

    let cmd_handler = handler::Builder::new().build(satellite_svc);

    // Constructing gRPC services
    let server_task = {
        let broker_service = BrokerService::new(cmd_handler, tlm_bus, last_tmiv_store);
        let broker_server = BrokerServer::new(broker_service);

        let tmtc_generic_c2a_server = TmtcGenericC2aServer::new(tmtc_generic_c2a_service);

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
        async fn set_serving<S: NamedService>(health_reporter: &mut HealthReporter, _: &S) {
            health_reporter.set_serving::<S>().await;
        }
        set_serving(&mut health_reporter, &broker_server).await;
        set_serving(&mut health_reporter, &tmtc_generic_c2a_server).await;
        let grpc_web_layer = GrpcWebLayer::new();
        let cors_layer = CorsLayer::new()
            .allow_methods([http::Method::GET, http::Method::POST])
            .allow_headers(tower_http::cors::Any)
            .allow_origin(tower_http::cors::Any);
        let trace_layer = TraceLayer::new_for_grpc();
        let layer = ServiceBuilder::new()
            .layer(trace_layer)
            .layer(cors_layer)
            .layer(grpc_web_layer);
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(broker::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(proto::tmtc_generic_c2a::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        let socket_addr = SocketAddr::new(args.broker_addr, args.broker_port);
        tracing::info!(message = "starting broker", %socket_addr);

        Server::builder()
            .accept_http1(true)
            .layer(layer)
            .add_service(broker_server)
            .add_service(tmtc_generic_c2a_server)
            .add_service(health_service)
            .add_service(reflection_service)
            .serve(socket_addr)
    };

    tokio::select! {
        ret = sat_tlm_reporter_task => Ok(ret?),
        ret = kble_socket_fut => Ok(ret?),
        ret = server_task => Ok(ret?),
    }
}
