use std::{pin::Pin, sync::Arc, time};
use tokio::sync::Mutex;

use crate::{
    registry::{CommandRegistry, FatCommandSchema, TelemetryRegistry},
    tco::{self, ParameterListWriter},
    tmiv,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use gaia_ccsds_c2a::{
    ccsds::{self, aos, tc},
    ccsds_c2a::{
        self,
        aos::{virtual_channel::Demuxer, SpacePacket},
        tc::{segment, space_packet},
    },
};
use gaia_tmtc::{
    tco_tmiv::{Tco, Tmiv},
    Handle,
};
use tracing::{debug, error, warn};

struct TmivBuilder {
    tlm_registry: TelemetryRegistry,
}

impl TmivBuilder {
    fn build(
        &self,
        plugin_received_time: time::SystemTime,
        space_packet_bytes: &[u8],
        space_packet: ccsds::SpacePacket<&[u8]>,
    ) -> Result<Vec<Tmiv>> {
        let plugin_received_time_secs = plugin_received_time
            .duration_since(time::UNIX_EPOCH)
            .expect("incorrect system clock")
            .as_secs();

        let space_packet = SpacePacket::from_generic(space_packet)
            .ok_or_else(|| anyhow!("space packet is too short"))?;
        let apid = space_packet.primary_header.apid();
        let tlm_id = space_packet.secondary_header.telemetry_id();
        let Some(telemetry) = self.tlm_registry.lookup(apid, tlm_id) else {
            return Err(anyhow!("unknown tlm_id: {tlm_id} from apid: {apid}"));
        };
        let channels = self
            .tlm_registry
            .find_channels(space_packet.secondary_header.destination_flags());
        let mut fields = vec![];
        tmiv::FieldsBuilder::new(&telemetry.schema).build(&mut fields, space_packet_bytes)?;
        let tmivs = channels
            .map(|channel| {
                let name = telemetry.build_tmiv_name(channel);
                Tmiv {
                    name: name.to_string(),
                    plugin_received_time: plugin_received_time_secs,
                    timestamp: Some(plugin_received_time.into()),
                    fields: fields.clone(),
                }
            })
            .collect();
        Ok(tmivs)
    }
}

struct CommandContext<'a> {
    tc_scid: u16,
    fat_schema: FatCommandSchema<'a>,
    tco: &'a Tco,
}

impl<'a> CommandContext<'a> {
    fn build_tc_segment(&self, data_field_buf: &mut [u8]) -> Result<usize> {
        let mut segment = segment::Builder::new(data_field_buf).unwrap();
        segment.use_default();

        let space_packet_bytes = segment.body_mut();
        let mut space_packet = space_packet::Builder::new(&mut space_packet_bytes[..]).unwrap();
        let tco_reader = tco::Reader::new(self.tco);
        let params_writer = ParameterListWriter::new(self.fat_schema.schema);
        space_packet.use_default();
        let ph = space_packet.ph_mut();
        ph.set_version_number(0); // always zero
        ph.set_apid(self.fat_schema.apid);
        let sh = space_packet.sh_mut();
        sh.set_command_id(self.fat_schema.command_id);
        sh.set_destination_type(self.fat_schema.destination_type);
        sh.set_execution_type(self.fat_schema.execution_type);
        if self.fat_schema.has_time_indicator {
            sh.set_time_indicator(tco_reader.time_indicator()?);
        } else {
            sh.set_time_indicator(0);
        }
        let user_data_len = params_writer.write_all(
            space_packet.user_data_mut(),
            tco_reader.parameters().into_iter(),
        )?;
        let space_packet_len = space_packet.finish(user_data_len);
        let segment_len = segment.finish(space_packet_len);
        Ok(segment_len)
    }

    async fn transmit_to<T>(&self, sync_and_channel_coding: &mut T) -> Result<()>
    where
        T: tc::SyncAndChannelCoding,
    {
        let vcid = 0; // FIXME: make this configurable
        let frame_type = tc::sync_and_channel_coding::FrameType::TypeBD;
        let sequence_number = 0; // In case of Type-BD, it's always zero.
        let mut data_field = vec![0u8; 1017]; // FIXME: hard-coded max size
        let segment_len = self.build_tc_segment(&mut data_field)?;
        data_field.truncate(segment_len);
        sync_and_channel_coding
            .transmit(self.tc_scid, vcid, frame_type, sequence_number, &data_field)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Service<T> {
    sync_and_channel_coding: Arc<Mutex<T>>,
    registry: Arc<CommandRegistry>,
    tc_scid: u16,
}

impl<T> Service<T>
where
    T: tc::SyncAndChannelCoding,
{
    async fn try_handle_command(&mut self, tco: &Tco) -> Result<bool> {
        let Some(fat_schema) = self.registry.lookup(&tco.name) else {
            return Ok(false);
        };
        let ctx = CommandContext {
            tc_scid: self.tc_scid,
            fat_schema,
            tco,
        };
        ctx.transmit_to(&mut *self.sync_and_channel_coding.lock().await)
            .await?;
        Ok(true)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn new<T, R>(
    aos_scid: u16,
    tc_scid: u16,
    tlm_registry: TelemetryRegistry,
    cmd_registry: impl Into<Arc<CommandRegistry>>,
    receiver: R,
    transmitter: T,
) -> (Service<T>, FopCommandService<T>, TelemetryReporter<R>)
where
    T: tc::SyncAndChannelCoding + Send + 'static,
    R: aos::SyncAndChannelCoding,
{
    let registry = cmd_registry.into();
    let transmitter = Arc::new(Mutex::new(transmitter));
    let fop = crate::fop1::Fop::new();
    let fop = Arc::new(Mutex::new(fop));
    let fop_command_service =
        FopCommandService::start(fop.clone(), tc_scid, transmitter.clone(), registry.clone());
    (
        Service {
            tc_scid,
            sync_and_channel_coding: transmitter,
            registry,
        },
        fop_command_service,
        TelemetryReporter {
            aos_scid,
            receiver,
            tmiv_builder: TmivBuilder { tlm_registry },
            fop,
        },
    )
}

#[async_trait]
impl<T> Handle<Arc<Tco>> for Service<T>
where
    T: tc::SyncAndChannelCoding + Clone + Send + Sync + 'static,
{
    type Response = Option<()>;

    async fn handle(&mut self, tco: Arc<Tco>) -> Result<Self::Response> {
        Ok(self.try_handle_command(&tco).await?.then_some(()))
    }
}

pub struct TelemetryReporter<R> {
    #[allow(unused)]
    aos_scid: u16,
    tmiv_builder: TmivBuilder,
    receiver: R,
    fop: Arc<Mutex<crate::fop1::Fop>>,
}

impl<R> TelemetryReporter<R>
where
    R: aos::SyncAndChannelCoding,
{
    pub async fn run<H>(mut self, mut tlm_handler: H) -> Result<()>
    where
        H: Handle<Arc<Tmiv>, Response = ()>,
    {
        let mut demuxer = Demuxer::default();
        loop {
            let tf_buf = self.receiver.receive().await?;
            let mut plugin_received_time = time::SystemTime::now();
            let tf: Option<ccsds_c2a::aos::TransferFrame<_>> = tf_buf.transfer_frame();
            let Some(tf) = tf else {
                let bytes = tf_buf.into_inner();
                warn!(
                    "transfer frame is too short ({} bytes): {:02x?}",
                    bytes.len(),
                    bytes
                );
                continue;
            };

            {
                let clcw = tf.trailer.into_ref().clone();
                let mut fop = self.fop.lock().await;
                if let Err(e) = fop.handle_clcw(clcw).await {
                    error!("failed to handle CLCW: {:?}", e);
                }
            }

            let incoming_scid = tf.primary_header.scid();
            if incoming_scid != self.aos_scid {
                warn!("unknown SCID: {incoming_scid}");
                continue;
            }
            let vcid = tf.primary_header.vcid();
            let channel = demuxer.demux(vcid);
            let frame_count = tf.primary_header.frame_count();
            if let Err(expected) = channel.synchronizer.next(frame_count) {
                warn!(
                    %vcid,
                    "some transfer frames has been dropped: expected frame count: {} but got {}",
                    expected, frame_count,
                );
                channel.defragmenter.reset();
            }
            if let Err(e) = channel.defragmenter.push(tf.data_unit_zone) {
                warn!(%vcid, "malformed M_PDU: {}", e);
                channel.synchronizer.reset();
                channel.defragmenter.reset();
                continue;
            }
            while let Some((space_packet_bytes, space_packet)) =
                channel.defragmenter.read_as_bytes_and_packet()
            {
                if space_packet.primary_header.is_idle_packet() {
                    debug!("skipping idle packet");
                } else {
                    match self.tmiv_builder.build(
                        plugin_received_time,
                        space_packet_bytes,
                        space_packet,
                    ) {
                        Ok(tmivs) => {
                            for tmiv in tmivs {
                                if let Err(e) = tlm_handler.handle(Arc::new(tmiv)).await {
                                    error!("failed to handle telemetry: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!(%vcid, "failed to build TMIV from space packet: {}", e);
                            channel.defragmenter.reset();
                            break;
                        }
                    };
                    // NOTE: workaround to avoid timestamp collision
                    plugin_received_time += time::Duration::from_nanos(1);
                }
                channel.defragmenter.advance();
            }
        }
    }
}

pub struct FopCommandService<T> {
    transmitter: Arc<Mutex<T>>,
    tc_scid: u16,
    fop: Arc<Mutex<crate::fop1::Fop>>,
    registry: Arc<CommandRegistry>,
}

impl<T: tc::SyncAndChannelCoding + Send + 'static> FopCommandService<T> {
    pub(crate) fn start(
        fop: Arc<Mutex<crate::fop1::Fop>>,
        tc_scid: u16,
        transmitter: Arc<Mutex<T>>,
        registry: Arc<CommandRegistry>,
    ) -> Self {
        let service = Self {
            transmitter,
            tc_scid,
            fop,
            registry,
        };

        tokio::spawn(Self::run_update(
            tc_scid,
            service.transmitter.clone(),
            service.fop.clone(),
        ));
        service
    }

    async fn run_update(
        tc_scid: u16,
        transmitter: Arc<Mutex<T>>,
        fop: Arc<Mutex<crate::fop1::Fop>>,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            //tracing::debug!("FopCommandService: update");
            while let Some(frame) = fop.lock().await.update() {
                tracing::debug!(
                    "FopCommandService: retransmitting {}",
                    frame.sequence_number
                );
                let mut transmitter = transmitter.lock().await;
                let vcid = 0;
                let _ = transmitter
                    .transmit(
                        tc_scid,
                        vcid,
                        frame.frame_type,
                        frame.sequence_number,
                        &frame.data_field,
                    )
                    .await;
            }
        }
    }
}

#[async_trait]
impl<T: tc::SyncAndChannelCoding + Send> gaia_tmtc::broker::FopCommandService
    for FopCommandService<T>
{
    async fn send_set_vr(&mut self, vr: u8) {
        let frame = {
            let mut fop = self.fop.lock().await;
            let frame = fop.set_vr(vr);
            match frame {
                Some(frame) => frame,
                None => {
                    //TODO: return error?
                    return;
                }
            }
        };

        let vcid = 0;
        let mut transmitter = self.transmitter.lock().await;
        let _ = transmitter
            .transmit(
                self.tc_scid,
                vcid,
                frame.frame_type,
                frame.sequence_number,
                &frame.data_field,
            )
            .await;
        //transmitter.
    }

    async fn send_ad_command(&mut self, tco: Tco) -> Result<u64> {
        let Some(fat_schema) = self.registry.lookup(&tco.name) else {
            return Err(anyhow!("unknown command: {}", tco.name));
        };
        let ctx = CommandContext {
            tc_scid: 0, // dummy
            fat_schema,
            tco: &tco,
        };
        let mut buf = vec![0u8; 1017]; // FIXME: hard-coded max size
        let len = ctx.build_tc_segment(&mut buf)?;
        buf.truncate(len);

        let mut fop = self.fop.lock().await;
        let frame = match fop.send_ad(buf) {
            None => {
                tracing::warn!("FOP is not ready");
                return Err(anyhow!("FOP is not ready"));
            }
            Some(frame) => frame,
        };

        let vcid = 0;
        let mut transmitter = self.transmitter.lock().await;
        let _ = transmitter
            .transmit(
                self.tc_scid,
                vcid,
                frame.frame_type,
                frame.sequence_number,
                &frame.data_field,
            )
            .await;

        Ok(frame.id)
    }

    async fn subscribe_frame_events(
        &mut self,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = gaia_tmtc::broker::FopFrameEvent> + Send>>> {
        use futures::StreamExt;
        let rx = self.fop.lock().await.subscribe_frame_events();
        let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|e| async {
            use crate::fop1::FrameEvent;
            use gaia_tmtc::broker::FopFrameEvent;
            let e = e.ok()?;
            let e = match e {
                FrameEvent::Transmit(id) => FopFrameEvent::Transmit(id),
                FrameEvent::Acknowledged(id) => FopFrameEvent::Acknowledged(id),
                FrameEvent::Retransmit(id) => FopFrameEvent::Retransmit(id),
                FrameEvent::Cancel(id) => FopFrameEvent::Cancel(id),
            };
            Some(e)
        });
        Ok(stream.boxed())
    }
}
