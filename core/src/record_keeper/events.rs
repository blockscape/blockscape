use primitives::{U256, JU256, Event, RawEvent, JRawEvent};
use std::collections::BTreeMap;
use std::mem::size_of;
use super::PlotID;

/// An event regarding the keeping of records, such as the introduction of a new block or shifting
/// state.
///
/// **Note:** notifications will only be sent once the changes to state have been applied unless
/// otherwise stated. This means that if there is a `NewBlock` message, a call to retrieve the block
/// will succeed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct PlotEvent {
    pub from: PlotID,
    pub to: PlotID,
    pub event: RawEvent
}
impl Event for PlotEvent {}

impl PlotEvent {
    /// Calculate the encoded size of this event in bytes.
    pub fn calculate_size(&self) -> usize {
        size_of::<PlotID>() * 2 +
        self.event.len() + 1
    }
}

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



#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JRecordEvent {
    NewBlock { uncled: bool, hash: JU256 },
    NewTxn { hash: JU256 },
    StateInvalidated { new_height: u64, after_height: u64, after_tick: u64}
}

impl From<RecordEvent> for JRecordEvent {
    fn from(e: RecordEvent) -> JRecordEvent {
        match e {
            RecordEvent::NewBlock{uncled, hash} => JRecordEvent::NewBlock{uncled, hash: hash.into()},
            RecordEvent::NewTxn{hash} => JRecordEvent::NewTxn{hash: hash.into()},
            RecordEvent::StateInvalidated{new_height, after_height, after_tick} => JRecordEvent::StateInvalidated{new_height, after_height, after_tick}
        }
    }
}

impl Into<RecordEvent> for JRecordEvent {
    fn into(self) -> RecordEvent {
        match self {
            JRecordEvent::NewBlock{uncled, hash} => RecordEvent::NewBlock{uncled, hash: hash.into()},
            JRecordEvent::NewTxn{hash} => RecordEvent::NewTxn{hash: hash.into()},
            JRecordEvent::StateInvalidated{new_height, after_height, after_tick} => RecordEvent::StateInvalidated{new_height, after_height, after_tick}
        }
    }
}


#[derive(Serialize, Deserialize)]
pub struct JPlotEvent {
    from: PlotID,
    to: PlotID,
    event: JRawEvent
}

impl From<PlotEvent> for JPlotEvent {
    fn from(e: PlotEvent) -> JPlotEvent {
        JPlotEvent {from: e.from, to: e.to, event: e.event.into()}
    }
}

impl Into<PlotEvent> for JPlotEvent {
    fn into(self) -> PlotEvent {
        PlotEvent {from: self.from, to: self.to, event: self.event.into()}
    }
}