use serde::de::DeserializeOwned;
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

/// An `Event` is an implementation defined type which will be used when processing the game to
/// determine how the game computation should be impacted. The final implementation should probably
/// be an enum, which would easily allow for multiple different kinds of events. Events may not
/// store references to external data as they may be brought into and out of existence at any time.
pub trait Event: Clone + Debug + DeserializeOwned + Send + Serialize + Sync + PartialEq + Eq + 'static {}


impl Event for Vec<u8> {}
pub type RawEvent = Vec<u8>;


/// `EventListener`s are designed to be notified of new events as they happen so the implementing
/// object does not have to regularly check if things have changed.
pub trait EventListener<E: Event>: Send + Sync {
    /// Notify will be called when a new event comes in.
    fn notify(&self, tick: u64, event: &E);
}


/// Lists of events stored by their tick
pub type Events<E: Event> = BTreeMap<u64, Vec<E>>;

pub fn add_event<E: Event>(events: &mut Events<E>, tick: u64, event: E) {
    let mut inserted_event = None;
    if let Some(ref mut list) = events.get_mut(&tick) {
        list.push(event);
    } else {
        inserted_event = Some(event);
    }
    if let Some(event) = inserted_event {
        let mut list = Vec::new();
        list.push(event);
        events.insert(tick, list);
    }
}

pub fn remove_event<E: Event>(events: &mut Events<E>, tick: u64, event: &E) -> bool {
    if let Some(ref mut list) = events.get_mut(&tick) {
        list.retain(|e| *e != *event); true
    } else { false }
}