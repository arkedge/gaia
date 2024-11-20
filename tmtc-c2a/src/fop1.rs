use anyhow::Result;
use gaia_ccsds_c2a::ccsds::tc::{self, clcw::CLCW};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::broadcast;

fn wrapping_le(a: u8, b: u8) -> bool {
    let diff = b.wrapping_sub(a);
    diff < 128
}

fn wrapping_lt(a: u8, b: u8) -> bool {
    a != b && wrapping_le(a, b)
}

fn remove_acknowledged_frames(
    queue: &mut VecDeque<SentFrame>,
    acknowledged_fsn: u8,
    on_acknowledge: impl Fn(u64),
) -> usize {
    let mut ack_count = 0;
    while !queue.is_empty() {
        let front = queue.front().unwrap();
        if wrapping_lt(front.sequence_number, acknowledged_fsn) {
            ack_count += 1;
            let frame = queue.pop_front().unwrap().frame;
            on_acknowledge(frame.id);
        } else {
            break;
        }
    }
    ack_count
}

#[derive(Clone, Copy)]
pub(crate) struct FarmState {
    pub(crate) next_expected_fsn: u8,
    pub(crate) lockout: bool,
    pub(crate) wait: bool,
    pub(crate) retransmit: bool,
}

enum FopState {
    Active(ActiveState),
    Retransmit(RetransmitState),
    Initial { expected_nr: Option<u8> },
}

struct SentFrame {
    frame: Arc<Frame>,
    sent_at: std::time::Instant,
    sequence_number: u8,
}

struct ActiveState {
    next_fsn: u8,
    sent_queue: VecDeque<SentFrame>,
}

struct RetransmitState {
    next_fsn: u8,
    retransmit_count: usize,
    retransmit_sent_queue: VecDeque<SentFrame>,
    retransmit_wait_queue: VecDeque<SentFrame>,
}

impl ActiveState {
    fn acknowledge(&mut self, acknowledged_fsn: u8, on_acknowledge: impl Fn(u64)) {
        remove_acknowledged_frames(&mut self.sent_queue, acknowledged_fsn, on_acknowledge);
    }

    fn send(
        &mut self,
        next_frame_id: &mut u64,
        data_field: Vec<u8>,
        on_transmit: impl Fn(u64),
    ) -> Option<Arc<Frame>> {
        let fsn = self.next_fsn;
        self.next_fsn = self.next_fsn.wrapping_add(1);
        let frame = Frame {
            id: *next_frame_id,
            frame_type: tc::sync_and_channel_coding::FrameType::TypeAD,
            sequence_number: fsn,
            data_field,
        };
        *next_frame_id += 1;
        let frame = Arc::new(frame);
        on_transmit(frame.id);
        self.sent_queue.push_back(SentFrame {
            frame: frame.clone(),
            sent_at: std::time::Instant::now(),
            sequence_number: fsn,
        });

        Some(frame)
    }

    fn timeout(&self) -> bool {
        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
        if let Some(head) = self.sent_queue.front() {
            if head.sent_at.elapsed() > TIMEOUT {
                return true;
            }
        }
        false
    }
}

impl RetransmitState {
    fn acknowledge(
        &mut self,
        acknowledged_fsn: u8,
        retransmit: bool,
        on_acknowledge: impl Fn(u64),
    ) -> bool {
        let ack_count = remove_acknowledged_frames(
            &mut self.retransmit_wait_queue,
            acknowledged_fsn,
            &on_acknowledge,
        ) + remove_acknowledged_frames(
            &mut self.retransmit_sent_queue,
            acknowledged_fsn,
            &on_acknowledge,
        );
        if ack_count > 0 {
            self.retransmit_count = 0;
        }

        if !retransmit {
            return self.retransmit_wait_queue.is_empty() && self.retransmit_sent_queue.is_empty();
        }

        if ack_count > 0 {
            self.redo_retransmit();
        }
        false
    }

    fn redo_retransmit(&mut self) {
        self.retransmit_count += 1;
        // prepend sent_queue to wait_queue
        // but the library doesn't provide "prepend" method...
        self.retransmit_sent_queue
            .append(&mut self.retransmit_wait_queue);
        std::mem::swap(
            &mut self.retransmit_sent_queue,
            &mut self.retransmit_wait_queue,
        );
    }

    fn update(&mut self) -> Option<Arc<Frame>> {
        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
        if let Some(head) = self.retransmit_sent_queue.front() {
            if head.sent_at.elapsed() > TIMEOUT {
                self.redo_retransmit();
            }
        }

        let mut next_retransmit = self.retransmit_wait_queue.pop_front()?;
        let frame = next_retransmit.frame.clone();
        next_retransmit.sent_at = std::time::Instant::now();
        self.retransmit_sent_queue.push_back(next_retransmit);
        Some(frame)
    }
}

pub(crate) struct Fop {
    next_frame_id: u64,
    state: FopState,
    last_received_farm_state: Option<FarmState>,
    event_sender: broadcast::Sender<FrameEvent>,
}

impl Fop {
    pub(crate) fn new() -> Self {
        let (event_sender, _) = broadcast::channel(16);
        Self {
            next_frame_id: 0,
            state: FopState::Initial { expected_nr: None },
            last_received_farm_state: None,
            event_sender,
        }
    }

    pub(crate) fn last_received_farm_state(&self) -> Option<&FarmState> {
        self.last_received_farm_state.as_ref()
    }

    pub(crate) fn next_fsn(&self) -> Option<u8> {
        match &self.state {
            FopState::Initial { expected_nr } => *expected_nr,
            FopState::Active(state) => Some(state.next_fsn),
            FopState::Retransmit(state) => Some(state.next_fsn),
        }
    }

    pub(crate) fn subscribe_frame_events(&self) -> broadcast::Receiver<FrameEvent> {
        self.event_sender.subscribe()
    }

    pub(crate) async fn handle_clcw(&mut self, clcw: CLCW) -> Result<()> {
        tracing::debug!("Received CLCW: {:?}", clcw);
        let farm_state = FarmState {
            next_expected_fsn: clcw.report_value(),
            lockout: clcw.lockout() != 0,
            wait: clcw.wait() != 0,
            retransmit: clcw.retransmit() != 0,
        };
        self.last_received_farm_state = Some(farm_state);

        let on_acknowledge = |frame_id| {
            self.event_sender
                .send(FrameEvent::Acknowledged(frame_id))
                .ok();
        };

        match &mut self.state {
            FopState::Initial { expected_nr } => {
                if Some(farm_state.next_expected_fsn) == *expected_nr && !farm_state.lockout {
                    tracing::info!("FOP initialized");
                    self.state = FopState::Active(ActiveState {
                        next_fsn: farm_state.next_expected_fsn,
                        sent_queue: VecDeque::new(),
                    });
                }
            }
            FopState::Active(state) => {
                state.acknowledge(farm_state.next_expected_fsn, on_acknowledge);
                if farm_state.retransmit {
                    self.state = FopState::Retransmit(RetransmitState {
                        next_fsn: state.next_fsn,
                        retransmit_count: 1,
                        retransmit_sent_queue: VecDeque::new(),
                        retransmit_wait_queue: std::mem::take(&mut state.sent_queue),
                    });
                }
            }
            FopState::Retransmit(state) => {
                let completed = state.acknowledge(
                    farm_state.next_expected_fsn,
                    farm_state.retransmit,
                    on_acknowledge,
                );
                if completed {
                    self.state = FopState::Active(ActiveState {
                        next_fsn: state.next_fsn,
                        sent_queue: VecDeque::new(),
                    });
                }
            }
        }

        if !farm_state.lockout {
            return Ok(());
        }

        //lockout
        let mut canceled_frames = VecDeque::new();
        match &mut self.state {
            FopState::Initial { .. } => {
                // do nothing
            }
            FopState::Active(state) => {
                canceled_frames.append(&mut state.sent_queue);
                self.state = FopState::Initial {
                    expected_nr: Some(state.next_fsn),
                };
            }
            FopState::Retransmit(state) => {
                canceled_frames.append(&mut state.retransmit_sent_queue);
                canceled_frames.append(&mut state.retransmit_wait_queue);
                self.state = FopState::Initial {
                    expected_nr: Some(state.next_fsn),
                };
            }
        }

        for frame in canceled_frames {
            self.event_sender
                .send(FrameEvent::Cancel(frame.frame.id))
                .ok();
        }

        Ok(())
    }

    pub(crate) fn set_vr(&mut self, vr: u8) -> Option<Frame> {
        tracing::info!("Setting VR to {}", vr);
        let mut canceled_frames = VecDeque::new();
        match &mut self.state {
            FopState::Initial { .. } => {
                // forget the previous setvr command
                // do nothing
            }
            FopState::Active(state) => {
                canceled_frames.append(&mut state.sent_queue);
            }
            FopState::Retransmit(state) => {
                canceled_frames.append(&mut state.retransmit_sent_queue);
                canceled_frames.append(&mut state.retransmit_wait_queue);
            }
        }

        for frame in canceled_frames {
            self.event_sender
                .send(FrameEvent::Cancel(frame.frame.id))
                .ok();
        }

        self.state = FopState::Initial {
            expected_nr: Some(vr),
        };
        let frame = Frame {
            //TODO: manage BC retransmission and frame id for setvr command
            //id: self.next_frame_id,
            id: 0,
            frame_type: tc::sync_and_channel_coding::FrameType::TypeBC,
            // TODO: frame number of setvr command???
            sequence_number: 0,
            data_field: vec![0x82, 0x00, vr],
        };
        Some(frame)
    }

    pub(crate) fn unlock(&mut self) -> Option<Frame> {
        let frame = Frame {
            //TODO: manage BC retransmission and frame id for setvr command
            //id: self.next_frame_id,
            id: 0,
            frame_type: tc::sync_and_channel_coding::FrameType::TypeBC,
            // TODO: frame number of setvr command???
            sequence_number: 0,
            data_field: vec![0x00],
        };
        Some(frame)
    }

    pub(crate) fn send_ad(&mut self, data_field: Vec<u8>) -> Option<Arc<Frame>> {
        let state = match &mut self.state {
            FopState::Active(state) => state,
            _ => return None,
        };

        state.send(&mut self.next_frame_id, data_field, |frame_id| {
            self.event_sender.send(FrameEvent::Transmit(frame_id)).ok();
        })
    }

    pub(crate) fn update(&mut self) -> Option<Arc<Frame>> {
        if let FopState::Active(state) = &mut self.state {
            if state.timeout() {
                self.state = FopState::Retransmit(RetransmitState {
                    next_fsn: state.next_fsn,
                    retransmit_count: 1,
                    retransmit_sent_queue: VecDeque::new(),
                    retransmit_wait_queue: std::mem::take(&mut state.sent_queue),
                });
            }
        }

        let frame = match &mut self.state {
            FopState::Retransmit(state) => state.update(),
            _ => None,
        };
        let frame = frame?;
        self.event_sender
            .send(FrameEvent::Retransmit(frame.id))
            .ok();
        Some(frame)
    }
}

pub struct Frame {
    pub id: u64,
    pub frame_type: tc::sync_and_channel_coding::FrameType,
    pub sequence_number: u8,
    pub data_field: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum FrameEvent {
    Transmit(u64),
    Acknowledged(u64),
    Retransmit(u64),
    Cancel(u64),
}
