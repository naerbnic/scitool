use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Inner {
    ref_count: AtomicUsize,
    waker: futures::task::AtomicWaker,
}

#[derive(Clone)]
pub struct OpenTracker(Arc<Inner>);

impl OpenTracker {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            ref_count: AtomicUsize::new(0),
            waker: futures::task::AtomicWaker::new(),
        }))
    }

    #[must_use]
    pub fn spawn_marker(&self) -> OpenMarker {
        self.0
            .ref_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        OpenMarker(self.0.clone())
    }

    pub fn wait_for_close(&self) {
        let curr_count = self.0.ref_count.load(Ordering::SeqCst);
        if curr_count == 0 {
            return;
        }
        panic!("You really should wait for all of the markers to be closed asynchronously");
    }
}

impl Default for OpenTracker {
    fn default() -> Self {
        Self::new()
    }
}

pub struct OpenMarker(Arc<Inner>);

impl Clone for OpenMarker {
    fn clone(&self) -> Self {
        self.0.ref_count.fetch_add(1, Ordering::SeqCst);
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for OpenMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("OpenMarker").finish()
    }
}

impl Drop for OpenMarker {
    fn drop(&mut self) {
        if self.0.ref_count.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.0.waker.wake();
        }
    }
}
