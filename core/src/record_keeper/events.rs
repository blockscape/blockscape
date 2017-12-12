use primitives::{U256, Event, RawEvent};
use super::PlotID;
use std::collections::BTreeMap;

/// An event regarding the keeping of records, such as the introduction of a new block or shifting
/// state.
///
/// **Note:** notifications will only be sent once the changes to state have been applied unless
/// otherwise stated. This means that if there is a `NewBlock` message, a call to retrieve the block
/// will succeed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum RecordEvent {
    /// A new block has been added, walk forward (or back, if back, then a state invalidated event
    /// will also be pushed out if relevant)
    NewBlock { uncled: bool, hash: U256 },
    /// A new transaction that has come into the system and is now pending
    NewTxn { hash: U256 },
    /// The state needs to be transitioned backwards, probably onto a new branch
    StateInvalidated { new_height: u64, after_height: u64, after_tick: u64 },
}
impl Event for RecordEvent {}


/// An event representing something which happened on or between plots.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct PlotEvent {
    from: PlotID,
    to: PlotID,
    event: RawEvent
}
impl Event for PlotEvent {}

/// Lists of events stored by their tick
pub type PlotEvents = BTreeMap<u64, Vec<PlotEvent>>;

pub fn add_event(events: &mut PlotEvents, tick: u64, event: PlotEvent) {
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

pub fn remove_event(events: &mut PlotEvents, tick: u64, event: &PlotEvent) -> bool {
    if let Some(ref mut list) = events.get_mut(&tick) {
        list.retain(|e| *e != *event); true
    } else { false }
}
