use anyhow::{anyhow, ensure, Result};
use futures::{SinkExt, TryStreamExt};
use gaia_ccsds_c2a::{
    ccsds::{
        aos,
        tc::{self, sync_and_channel_coding::FrameType},
    },
    ccsds_c2a,
};
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    sync::{broadcast, mpsc, oneshot},
};
use tracing::{error, info};

pub fn new() -> (Link, Socket) {
    let (cmd_tx, cmd_rx) = mpsc::channel(1);
    let (tlm_tx, _) = broadcast::channel(10);
    let link = Link {
        cmd_tx,
        tlm_tx: tlm_tx.clone(),
    };
    let socket = Socket { cmd_rx, tlm_tx };
    (link, socket)
}

pub struct Socket {
    cmd_rx: mpsc::Receiver<(Vec<u8>, oneshot::Sender<Result<()>>)>,
    tlm_tx: broadcast::Sender<Vec<u8>>,
}

impl Socket {
    pub async fn serve(mut self, addr: impl ToSocketAddrs) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        loop {
            let accept_fut = listener.accept();
            let leak_fut = async {
                loop {
                    if let Some((_, resp_tx)) = self.cmd_rx.recv().await {
                        if let Err(e) =
                            resp_tx.send(Err(anyhow!("kble socket to satellite is not ready")))
                        {
                            break e;
                        }
                    }
                }
            };
            let (incoming, addr) = tokio::select! {
                accept = accept_fut => accept.map_err(|_| anyhow!("response receiver has gone"))?,
                resp_res = leak_fut => return resp_res,
            };
            info!("accept kble connection from {addr}");
            let wss = tokio_tungstenite::accept_async(incoming).await?;
            let (mut sink, mut stream) = kble_socket::from_tungstenite(wss);
            let uplink = async {
                loop {
                    let (cmd_bytes, resp_tx) = self
                        .cmd_rx
                        .recv()
                        .await
                        .ok_or_else(|| anyhow!("command sender has gone"))?;
                    let res = sink.send(cmd_bytes.into()).await;
                    resp_tx
                        .send(res)
                        .map_err(|_| anyhow!("response receiver has gone"))?;
                }
            };
            let downlink = async {
                loop {
                    let Some(tlm_bytes) = stream.try_next().await? else {
                        break;
                    };
                    self.tlm_tx.send(tlm_bytes.into())?;
                }
                anyhow::Ok(())
            };
            let res = tokio::select! {
                res = uplink => res,
                res = downlink => res,
            };
            if let Err(e) = res {
                error!("kble socket error: {e}")
            }
            sink.close().await?;
        }
    }
}

pub struct Link {
    cmd_tx: mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<()>>)>,
    tlm_tx: broadcast::Sender<Vec<u8>>,
}

impl Link {
    pub fn uplink(&self) -> Uplink {
        Uplink {
            cmd_tx: self.cmd_tx.clone(),
        }
    }

    pub fn downlink(&self) -> Downlink {
        Downlink {
            tlm_rx: self.tlm_tx.subscribe(),
        }
    }
}

#[derive(Debug)]
pub struct Downlink {
    tlm_rx: broadcast::Receiver<Vec<u8>>,
}

#[async_trait::async_trait]
impl aos::SyncAndChannelCoding for Downlink {
    async fn receive(&mut self) -> Result<aos::sync_and_channel_coding::TransferFrameBuffer> {
        loop {
            match self.tlm_rx.recv().await {
                Ok(bytes) => {
                    return Ok(aos::sync_and_channel_coding::TransferFrameBuffer::new(
                        bytes,
                    ))
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue, // NOTE: should report data lost?
                Err(e) => {
                    return Err(anyhow::Error::from(e)
                        .context("failed to receive telemetry bytes from broadcast channel"))
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Uplink {
    cmd_tx: mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<()>>)>,
}

#[async_trait::async_trait]
impl tc::SyncAndChannelCoding for Uplink {
    async fn transmit(
        &mut self,
        scid: u16,
        vcid: u8,
        frame_type: FrameType,
        sequence_number: u8,
        data: &[u8],
    ) -> Result<()> {
        let tf_bytes = build_tf(scid, vcid, frame_type, sequence_number, data)?;
        let (resp_tx, resp_rx) = oneshot::channel();
        self.cmd_tx.send((tf_bytes, resp_tx)).await?;
        resp_rx.await??;
        Ok(())
    }
}

fn build_tf(
    scid: u16,
    vcid: u8,
    frame_type: FrameType,
    sequence_number: u8,
    data: &[u8],
) -> Result<Vec<u8>> {
    let mut tf_bytes = vec![0u8; ccsds_c2a::tc::transfer_frame::MAX_SIZE];
    let mut tf_fecw = ccsds_c2a::tc::transfer_frame::Builder::new(&mut *tf_bytes).unwrap();
    let mut tf = tf_fecw.bare_mut().unwrap();
    tf.set_scid(scid);
    tf.set_vcid(vcid);
    tf.set_bypass_flag(frame_type.bypass_flag());
    tf.set_control_command_flag(frame_type.control_command_flag());
    tf.set_frame_sequence_number(sequence_number);
    let data_field = tf.data_field_mut();
    ensure!(data.len() <= data_field.len(), "too large data");
    data_field[..data.len()].copy_from_slice(data);
    let bare_len = tf.finish(data.len());
    let tf_len = tf_fecw.finish(bare_len);
    tf_bytes.truncate(tf_len);
    Ok(tf_bytes)
}
