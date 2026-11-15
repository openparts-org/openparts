//! Blocking work (HTTP, file scans) off the UI thread.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

/// Wakes the UI when a job finishes (egui only redraws on input or
/// request). A no-op in tests.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
pub fn no_waker() -> Waker {
    Arc::new(|| {})
}

/// One background computation's result, polled each frame.
pub struct Job<T> {
    rx: Receiver<Result<T, String>>,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(waker: &Waker, f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let waker = waker.clone();
        std::thread::spawn(move || {
            let _ = tx.send(f());
            waker();
        });
        Job { rx }
    }

    /// The result once it's ready (a job whose thread panicked is an
    /// error).
    pub fn poll(&self) -> Option<Result<T, String>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("background task failed".into())),
        }
    }

    /// Waits for the result (tests).
    #[cfg(test)]
    pub fn wait(&self) -> Result<T, String> {
        self.rx
            .recv()
            .unwrap_or_else(|_| Err("background task failed".into()))
    }
}
