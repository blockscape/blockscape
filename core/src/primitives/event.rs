use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// An `Event` is an implementation defined type which will be used when processing the game to
/// determine how the game computation should be impacted. The final implementation should probably
/// be an enum, which would easily allow for multiple different kinds of events. Events may not
/// store references to external data as they may be brought into and out of existence at any time.
pub trait Event: Clone + Debug + DeserializeOwned + Send + Serialize + Sync + 'static {}


/// `EventListener`s are designed to be notified of new events as they happen so the implementing
/// object does not have to regularly check if things have changed.
pub trait EventListener<E: Event>: Send + Sync {
    /// Notify will be called when a new event comes in.
    fn notify(&self, tick: u64, event: &E);
}


/// A set of events that all happened at the same time.
pub struct Events<E: Event> {
    pub tick: u64,
    pub events: Vec<E> // how do we know what type of event to deserialize as; could use an enum as a template parameter for record keeper
}


impl<E: Event> Deref for Events<E> {
    type Target = Vec<E>;
    fn deref(&self) -> &Vec<E> {
        &self.events
    }
}

impl<E: Event> DerefMut for Events<E> {
    fn deref_mut(&mut self) -> &mut Vec<E> {
        &mut self.events
    }
}