use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// One token per operation, shared only with that operation's runtime clone.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_requested(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
