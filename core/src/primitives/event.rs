use bin::*;
use std::fmt::Debug;
use futures::prelude::*;
use futures::sync::mpsc::Sender;
use futures::future::join_all;
use futures::future::*;
use std::collections::BTreeMap;

/// An `Event` is an implementation defined type which will be used when processing the game to
/// determine how the game computation should be impacted. The final implementation should probably
/// be an enum, which would easily allow for multiple different kinds of events. Events may not
/// store references to external data as they may be brought into and out of existence at any time.
pub trait Event: Debug + Send + Sync + Clone + 'static {}

impl Event for Bin {}
pub type RawEvent = Bin;
pub type JRawEvent = JBin;

/// Lists of events stored by their tick
pub type RawEvents = BTreeMap<u64, Vec<RawEvent>>;

/// Add an event to a PlotEvents object. Returns true if a new entry was added, false if it was a
/// duplicate. Will not add duplicate entries.
pub fn add_event(events: &mut RawEvents, tick: u64, event: RawEvent) -> bool {
    // if there is already a list of events for this tick, append to it
    if let Some(event_list) = events.get_mut(&tick) {
        if !event_list.contains(&event) {
            // the event is not already stored for that tick
            event_list.push(event.clone());
            return true; //we have added it successfully
        } else { return false; }
    }

    // if not, then we need to create a new entry
    let mut event_list = Vec::new();
    event_list.push(event);
    events.insert(tick, event_list);
    true
}

/// Remove an event from a PlotEvents object. Returns true if the event was removed.
pub fn remove_event(events: &mut RawEvents, tick: u64, event: &RawEvent) -> bool {
    let mut del_tick = false;
    let removed = if let Some(event_list) = events.get_mut(&tick) {
        let initial_size = event_list.len();
        event_list.retain(|e| e != event);
        if event_list.len() == 0 { del_tick = true; }
        event_list.len() < initial_size
    } else { false };

    if del_tick { events.remove(&tick).unwrap(); }
    removed
}

/// A set of listeners who are ready to receive events. This is designed to be a simple way to
/// manage a list and to notify all of them at once of something which has happened.
pub struct ListenerPool<E: Event>(Vec<Sender<E>>);

/*impl<E: Event> Deref for ListenerPool<E> {
    type Target = Vec<Sender<E>>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<E: Event> DerefMut for ListenerPool<E> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}*/

impl<E: Event> ListenerPool<E> {
    /// Create a new, empty, listener pool.
    pub fn new() -> ListenerPool<E> {
        ListenerPool(Vec::new())
    }

    /// Register a new listener with the pool.
    pub fn register(&mut self, listener: Sender<E>) {
        self.0.push(listener);
    }

    /// Notify all listeners, and return a future that resolves when the messages have all been
    /// sent, including the number of listeners we successfully sent messages to.
    pub fn notify(&mut self, event: &E) -> u32 {

        let drn: Vec<Sender<E>> = self.0.drain(..).collect();

        let f1 = join_all(drn.into_iter().map(
            |l| l.send(event.clone()).map(|lm| Some(lm)).or_else(|_| Ok::<Option<Sender<E>>, ()>(None))
        ));

        Box::new(f1.and_then(|r| {
            self.0.extend(r.into_iter().filter_map(|x| x));
            ok(self.0.len() as u32)
        })).wait().unwrap()
    }
}