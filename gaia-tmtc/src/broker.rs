use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use futures::prelude::*;
use gaia_stub::tco_tmiv::Tco;
use std::pin::Pin;
use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;
use tonic::{Request, Response, Status, Streaming};

use super::telemetry::{self, LastTmivStore};

pub use gaia_stub::broker::*;

pub struct BrokerService<C, F> {
    cmd_handler: Mutex<C>,
    fop_command_service: Mutex<F>,
    tlm_bus: telemetry::Bus,
    last_tmiv_store: Arc<LastTmivStore>,
}

impl<C, F> BrokerService<C, F> {
    pub fn new(
        cmd_service: C,
        fop_command_service: F,
        tlm_bus: telemetry::Bus,
        last_tmiv_store: Arc<LastTmivStore>,
    ) -> Self {
        Self {
            cmd_handler: Mutex::new(cmd_service),
            fop_command_service: Mutex::new(fop_command_service),
            tlm_bus,
            last_tmiv_store,
        }
    }
}

use async_trait::async_trait;
pub enum FopFrameEvent {
    Transmit(u64),
    Acknowledged(u64),
    Retransmit(u64),
    Cancel(u64),
}

#[derive(Default)]
pub struct ClcwInfo {
    pub lockout: bool,
    pub wait: bool,
    pub retransmit: bool,
    pub next_expected_fsn: u8,
}

pub enum StateSummary {
    Initial,
    Active,
    Retransmit { retransmit_count: u64 },
}

pub struct FopStatus {
    pub last_clcw: Option<ClcwInfo>,
    pub state_summary: StateSummary,
    pub next_fsn: Option<u8>,
}

#[async_trait]
pub trait FopCommandService {
    async fn send_set_vr(&self, value: u8);

    async fn clear(&self);

    async fn send_unlock(&self);

    async fn send_ad_command(&self, tco: Tco) -> Result<u64>;

    async fn subscribe_frame_events(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = FopFrameEvent> + Send + Sync>>>;

    async fn get_fop_status(&self) -> Result<FopStatus>;
}

#[tonic::async_trait]
impl<C, F> broker_server::Broker for BrokerService<C, F>
where
    C: super::Handle<Arc<Tco>> + Send + Sync + 'static,
    C::Response: Send + 'static,
    F: FopCommandService + Send + Sync + 'static,
{
    type OpenCommandStreamStream =
        stream::BoxStream<'static, Result<CommandStreamResponse, Status>>;
    type OpenTelemetryStreamStream =
        stream::BoxStream<'static, Result<TelemetryStreamResponse, Status>>;
    type SubscribeFopFrameEventsStream =
        stream::BoxStream<'static, Result<gaia_stub::broker::FopFrameEvent, Status>>;

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
    async fn post_set_vr(
        &self,
        request: Request<PostSetVrRequest>,
    ) -> Result<Response<PostSetVrResponse>, tonic::Status> {
        let message = request.into_inner();
        let value = message.vr;
        self.fop_command_service
            .lock()
            .await
            .send_set_vr(value as _)
            .await;
        Ok(Response::new(PostSetVrResponse {}))
    }

    #[tracing::instrument(skip(self))]
    async fn post_unlock(
        &self,
        _request: Request<PostUnlockRequest>,
    ) -> Result<Response<PostUnlockResponse>, tonic::Status> {
        self.fop_command_service.lock().await.send_unlock().await;
        Ok(Response::new(PostUnlockResponse {}))
    }

    #[tracing::instrument(skip(self))]
    async fn post_ad_command(
        &self,
        request: Request<PostAdCommandRequest>,
    ) -> Result<Response<PostAdCommandResponse>, tonic::Status> {
        let message = request.into_inner();

        let tco = message
            .tco
            .ok_or_else(|| Status::invalid_argument("tco is required"))?;

        fn internal_error<E: Debug>(e: E) -> Status {
            Status::internal(format!("{:?}", e))
        }
        let id = self
            .fop_command_service
            .lock()
            .await
            .send_ad_command(tco)
            .await
            .map_err(internal_error)?;

        tracing::info!("AD command sent");
        Ok(Response::new(PostAdCommandResponse {
            success: true,
            frame_id: id,
        }))
    }

    #[tracing::instrument(skip(self))]
    async fn clear_ad(
        &self,
        _request: tonic::Request<ClearAdRequest>,
    ) -> Result<tonic::Response<ClearAdResponse>, tonic::Status> {
        self.fop_command_service.lock().await.clear().await;
        Ok(Response::new(ClearAdResponse {}))
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

    #[tracing::instrument(skip(self))]
    async fn subscribe_fop_frame_events(
        &self,
        _request: tonic::Request<SubscribeFopFrameEventsRequest>,
    ) -> Result<tonic::Response<Self::SubscribeFopFrameEventsStream>, tonic::Status> {
        use futures::StreamExt;
        let stream = self
            .fop_command_service
            .lock()
            .await
            .subscribe_frame_events()
            .await
            .map_err(|_| Status::internal("failed to subscribe frame events"))?;
        use gaia_stub::broker::fop_frame_event::EventType;
        let stream = stream.map(|e| {
            let (frame_id, event_type) = match e {
                FopFrameEvent::Transmit(id) => (id, EventType::Transmit),
                FopFrameEvent::Acknowledged(id) => (id, EventType::Acknowledged),
                FopFrameEvent::Retransmit(id) => (id, EventType::Retransmit),
                FopFrameEvent::Cancel(id) => (id, EventType::Cancel),
            };
            Ok(gaia_stub::broker::FopFrameEvent {
                frame_id,
                event_type: event_type.into(),
            })
        });
        Ok(Response::new(stream.boxed()))
    }

    #[tracing::instrument(skip(self))]
    async fn get_fop_status(
        &self,
        _request: tonic::Request<GetFopStatusRequest>,
    ) -> Result<tonic::Response<GetFopStatusResponse>, tonic::Status> {
        let state = self
            .fop_command_service
            .lock()
            .await
            .get_fop_status()
            .await
            .map_err(|_| Status::internal("failed to get fop state"))?;
        let received_clcw = state.last_clcw.is_some();
        let clcw = state.last_clcw.unwrap_or_default();

        use gaia_stub::broker::{
            fop_state::{RetransmitState, State},
            FopState,
        };
        let fop_state = match state.state_summary {
            StateSummary::Initial => State::Initial(()),
            StateSummary::Active => State::Active(()),
            StateSummary::Retransmit { retransmit_count } => {
                State::Retransmit(RetransmitState { retransmit_count })
            }
        };
        let fop_state = FopState {
            state: Some(fop_state),
        };

        let resp = GetFopStatusResponse {
            received_clcw,
            lockout_flag: clcw.lockout,
            wait_flag: clcw.wait,
            retransmit_flag: clcw.retransmit,
            next_expected_sequence_number: clcw.next_expected_fsn as _,
            has_next_sequence_number: state.next_fsn.is_some(),
            next_sequence_number: state.next_fsn.unwrap_or_default() as _,
            fop_state: Some(fop_state),
        };

        Ok(Response::new(resp))
    }
}
