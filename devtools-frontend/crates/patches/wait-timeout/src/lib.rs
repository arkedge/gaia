//! A crate to wait on a child process with a particular timeout.

use std::io;
use std::process::{Child, ExitStatus};
use std::time::Duration;

/// Extension methods for the standard `std::process::Child` type.
pub trait ChildExt {
    /// Deprecated, use `wait_timeout` instead.
    #[doc(hidden)]
    fn wait_timeout_ms(&mut self, ms: u32) -> io::Result<Option<ExitStatus>> {
        self.wait_timeout(Duration::from_millis(ms as u64))
    }

    /// Wait for this child to exit, timing out after the duration `dur` has elapsed.
    fn wait_timeout(&mut self, dur: Duration) -> io::Result<Option<ExitStatus>>;
}

impl ChildExt for Child {
    fn wait_timeout(&mut self, dur: Duration) -> io::Result<Option<ExitStatus>> {
        drop(self.stdin.take());
        imp::wait_timeout(self, dur)
    }
}

#[cfg(any(unix, windows))]
mod imp {
    use std::cmp;
    use std::io;
    use std::process::{Child, ExitStatus};
    use std::time::{Duration, Instant};

    pub fn wait_timeout(child: &mut Child, dur: Duration) -> io::Result<Option<ExitStatus>> {
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(Some(status));
            }

            let elapsed = start.elapsed();
            if elapsed >= dur {
                return Ok(None);
            }

            std::thread::sleep(cmp::min(dur - elapsed, Duration::from_millis(10)));
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod imp {
    use std::io;
    use std::process::{Child, ExitStatus};
    use std::time::Duration;

    pub fn wait_timeout(_child: &mut Child, _dur: Duration) -> io::Result<Option<ExitStatus>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "waiting on child processes is not supported on this target",
        ))
    }
}
