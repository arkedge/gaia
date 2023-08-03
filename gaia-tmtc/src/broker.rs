use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use futures::prelude::*;
use gaia_stub::tco_tmiv::Tco;
use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;
use tonic::{Request, Response, Status, Streaming};

use super::telemetry::{self, LastTmivStore};

pub use gaia_stub::broker::*;

pub struct BrokerService<C> {
    cmd_handler: Mutex<C>,
    tlm_bus: telemetry::Bus,
    last_tmiv_store: Arc<LastTmivStore>,
}

impl<C> BrokerService<C> {
    pub fn new(
        cmd_service: C,
        tlm_bus: telemetry::Bus,
        last_tmiv_store: Arc<LastTmivStore>,
    ) -> Self {
        Self {
            cmd_handler: Mutex::new(cmd_service),
            tlm_bus,
            last_tmiv_store,
        }
    }
}

#[tonic::async_trait]
impl<C> broker_server::Broker for BrokerService<C>
where
    C: super::Handle<Arc<Tco>> + Send + Sync + 'static,
    C::Response: Send + 'static,
{
    type OpenCommandStreamStream =
        stream::BoxStream<'static, Result<CommandStreamResponse, Status>>;
    type OpenTelemetryStreamStream =
        stream::BoxStream<'static, Result<TelemetryStreamResponse, Status>>;

    #[tracing::instrument(skip(self))]
    async fn post_command(
        &self,
        request: Request<PostCommandRequest>,
    ) -> Result<Response<PostCommandResponse>, tonic::Status> {
        let message = request.into_inner();

        let tco = message
            .tco
            .ok_or_else(|| Status::invalid_argument("tco is required"))?;

        fn internal_error<E: Debug>(e: E) -> Status {
            Status::internal(format!("{:?}", e))
        }
        self.cmd_handler
            .lock()
            .await
            .handle(Arc::new(tco))
            .await
            .map_err(internal_error)?;

        Ok(Response::new(PostCommandResponse {}))
    }

    #[tracing::instrument(skip(self))]
    async fn open_telemetry_stream(
        &self,
        _request: tonic::Request<TelemetryStreamRequest>,
    ) -> Result<tonic::Response<Self::OpenTelemetryStreamStream>, tonic::Status> {
        let rx = self.tlm_bus.subscribe();
        let stream = BroadcastStream::new(rx)
            .map_ok(move |tmiv| TelemetryStreamResponse {
                tmiv: Some(tmiv.as_ref().clone()),
            })
            .map_err(|_| Status::data_loss("stream was lagged"));
        Ok(Response::new(Box::pin(stream)))
    }

    #[tracing::instrument(skip(self))]
    async fn open_command_stream(
        &self,
        _request: Request<Streaming<CommandStreamRequest>>,
    ) -> Result<Response<Self::OpenCommandStreamStream>, tonic::Status> {
        Err(tonic::Status::unimplemented("needless"))
    }

    #[tracing::instrument(skip(self))]
    async fn post_telemetry(
        &self,
        _request: tonic::Request<PostTelemetryRequest>,
    ) -> Result<tonic::Response<PostTelemetryResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("needless"))
    }

    #[tracing::instrument(skip(self))]
    async fn get_last_received_telemetry(
        &self,
        request: Request<GetLastReceivedTelemetryRequest>,
    ) -> Result<Response<GetLastReceivedTelemetryResponse>, Status> {
        let message = request.get_ref();
        let tmiv = self
            .last_tmiv_store
            .get(&message.telemetry_name)
            .await
            .map_err(|_| Status::invalid_argument("invalid telemetry name"))?;
        if let Some(tmiv) = tmiv {
            Ok(Response::new(GetLastReceivedTelemetryResponse {
                tmiv: Some(tmiv.as_ref().clone()),
            }))
        } else {
            Err(Status::not_found("not received yet"))
        }
    }
}
