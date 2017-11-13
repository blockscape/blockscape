use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

/// An `Event` is an implementation defined type which will be used when processing the game to
/// determine how the game computation should be impacted. The final implementation should probably
/// be an enum, which would easily allow for multiple different kinds of events. Events may not
/// store references to external data as they may be brought into and out of existence at any time.
pub trait Event: Clone + Debug + Send + Sync + 'static {}

impl Event for Vec<u8> {}
pub type RawEvent = Vec<u8>;


/// `EventListener`s are designed to be notified of new events as they happen so the implementing
/// object does not have to regularly check if things have changed.
pub trait EventListener<E: Event>: Send + Sync {
    /// Notify will be called when a new event comes in.
    fn notify(&self, tick: u64, event: E);
}