use primitives::{Block, JBlock, Txn, JTxn, Event, RawEvent, JRawEvent};
use std::collections::BTreeSet;
use std::mem::size_of;
use super::PlotID;
use bincode;
use serde::de::DeserializeOwned;
use serde::Serialize;

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
    NewBlock { uncled: bool, fresh: bool, block: Block },
    /// A new transaction that has come into the system and is now pending
    NewTxn { fresh: bool, txn: Txn },
    /// The state needs to be transitioned backwards, probably onto a new branch
    StateInvalidated { new_height: u64, after_height: u64 },
}
impl Event for RecordEvent {}


/// An event representing something which happened on or between plots. This is not stored directly
/// in the database, but the information is used to determine where the raw event will be stored.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct PlotEvent {
    pub from: PlotID,
    pub to: BTreeSet<PlotID>,
    pub tick: u64,
    pub event: RawEvent
}
impl Event for PlotEvent {}

impl PlotEvent {
    /// Calculate the encoded size of this event in bytes.
    pub fn calculate_size(&self) -> usize {
        size_of::<PlotID>() * (2 + self.to.len()) +
        self.event.len() + 1
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DePlotEvent<E> where E: Event {
    pub from: PlotID,
    pub to: BTreeSet<PlotID>,
    pub tick: u64,
    pub event: E
}
impl<E: Event> Event for DePlotEvent<E> {}

impl<E> DePlotEvent<E> where E: Event + DeserializeOwned {
    pub fn deserialize(e: &PlotEvent) -> Result<DePlotEvent<E>, bincode::Error> {
        Ok(DePlotEvent {
            from: e.from,
            to: e.to.clone(),
            tick: e.tick,
            event: bincode::deserialize(&e.event)?
        })
    }
}

impl<E> DePlotEvent<E> where E: Event + Serialize {
    pub fn serialize(&self) -> Result<PlotEvent, bincode::Error> {
        Ok(PlotEvent {
            from: self.from,
            to: self.to.clone(),
            tick: self.tick,
            event: bincode::serialize(&self.event, bincode::Infinite)?
        })
    }
}



#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JRecordEvent {
    NewBlock { uncled: bool, fresh: bool, block: JBlock },
    NewTxn { fresh: bool, txn: JTxn },
    StateInvalidated { new_height: u64, after_height: u64}
}

impl From<RecordEvent> for JRecordEvent {
    fn from(e: RecordEvent) -> JRecordEvent {
        match e {
            RecordEvent::NewBlock{uncled, fresh, block} => JRecordEvent::NewBlock{uncled, fresh, block: block.into()},
            RecordEvent::NewTxn{fresh, txn} => JRecordEvent::NewTxn{fresh, txn: txn.into()},
            RecordEvent::StateInvalidated{new_height, after_height} => JRecordEvent::StateInvalidated{new_height, after_height}
        }
    }
}

impl Into<RecordEvent> for JRecordEvent {
    fn into(self) -> RecordEvent {
        match self {
            JRecordEvent::NewBlock{uncled, fresh, block} => RecordEvent::NewBlock{uncled, fresh, block: block.into()},
            JRecordEvent::NewTxn{fresh, txn} => RecordEvent::NewTxn{fresh, txn: txn.into()},
            JRecordEvent::StateInvalidated{new_height, after_height} => RecordEvent::StateInvalidated{new_height, after_height}
        }
    }
}


#[derive(Serialize, Deserialize)]
pub struct JPlotEvent {
    from: PlotID,
    to: BTreeSet<PlotID>,
    tick: u64,
    event: JRawEvent
}

impl From<PlotEvent> for JPlotEvent {
    fn from(e: PlotEvent) -> JPlotEvent {
        JPlotEvent {from: e.from, to: e.to, tick: e.tick, event: e.event.into()}
    }
}

impl Into<PlotEvent> for JPlotEvent {
    fn into(self) -> PlotEvent {
        PlotEvent {from: self.from, to: self.to, tick: self.tick, event: self.event.into()}
    }
}