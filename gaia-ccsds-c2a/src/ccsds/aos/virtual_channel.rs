use super::transfer_frame::FrameCount;

#[derive(Debug, Default)]
pub struct Synchronizer {
    counter: Option<FrameCount>,
}

impl Synchronizer {
    pub fn next(&mut self, frame_count: FrameCount) -> Result<(), FrameCount> {
        if let Some(counter) = self.counter {
            let is_contiguous = frame_count.is_next_to(counter);
            self.counter = Some(frame_count);
            if is_contiguous {
                Ok(())
            } else {
                Err(counter.next())
            }
        } else {
            self.counter = Some(frame_count);
            Ok(())
        }
    }

    pub fn reset(&mut self) {
        self.counter = None;
    }
}
