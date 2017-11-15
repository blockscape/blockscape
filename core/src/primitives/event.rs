use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};

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
    fn notify(&self, tick: u64, event: &E);
}

/// A set of listeners who are ready to receive events. This is designed to be a simple way to
/// manage a list and to notify all of them at once of something which has happened.
pub struct ListenerPool<E: Event>(Vec<Weak<EventListener<E>>>);

impl<E: Event> Deref for ListenerPool<E> {
    type Target = Vec<Weak<EventListener<E>>>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<E: Event> DerefMut for ListenerPool<E> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<E: Event> ListenerPool<E> {
    /// Create a new, empty, listener pool.
    pub fn new() -> ListenerPool<E> {
        ListenerPool(Vec::new())
    }

    /// Register a new listener with the pool.
    pub fn register(&mut self, listener: &Arc<EventListener<E>>) {
        self.clean();
        self.push(Arc::downgrade(listener));
    }

    /// Remove any listeners for which our references are no longer relevant.
    pub fn clean(&mut self) {
        self.retain(|l| l.upgrade().is_some());
    }

    /// Notify all listeners, and return the number of listeners we successfully sent messages to.
    pub fn notify(&self, tick: u64, event: &E) -> u32 {
        let mut count: u32 = 0;
        for l in &self.0 {
            if let Some(listener) = l.upgrade() {
                count += 1;
                listener.notify(tick, &event);
            }
        } count
    }
}