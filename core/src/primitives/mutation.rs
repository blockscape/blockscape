use bin::*;
use record_keeper::{PlotID, PlotEvent, JPlotEvent};
use std::mem::size_of;
use primitives::{U160, JU160};

/// A single change to the database, a mutation may be the composite of multiple changes. This is
/// designed as a simple structure which the outer world can use to store the changes which should
/// not know anything about the database.
#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub enum Change {
    Admin { key: Bin, value: Option<Bin> },
    BlockReward { id: U160, proof: Bin },
    Event { id: PlotID, tick: u64, event: PlotEvent },
    NewValidator { pub_key: Bin },
    Slash { id: U160, amount: u64, proof: Bin }
}

impl Change {
    /// Calculate the encoded size of this change in bytes.
    pub fn calculate_size(&self) -> usize {
        8 + match self {
            &Change::Admin{ref key, ref value} => {
                key.len() + 1 +
                if let Some(a) = value.as_ref() { a.len() } else { 0 } + 2
            },
            &Change::BlockReward{ref proof, ..} => 20 + proof.len() + 1,
            &Change::Event{ref event, ..} => size_of::<PlotID>() + 8 + event.calculate_size(),
            &Change::NewValidator{ref pub_key} => pub_key.len() + 1,
            &Change::Slash{ref proof, ..} => 20 + 8 + proof.len() + 1
        }
    }
}



/// A composition of changes which are to be atomically applied to the database. In a few places,
/// certain actions on contras will fail by assertion, I believe any such error should be a result
/// of a coding mistake and should not need to be determined at runtime.
/// TODO: consider storing a HashSet<Rc<Change>> to save on cloning.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Mutation {
    contra: bool, // true iff it is a normal mutation, else it is a contra mutation.
    pub changes: Vec<Change>
}

impl Mutation {
    /// Create a new, empty mutation.
    pub fn new() -> Mutation {
        Mutation {contra: false, changes: Vec::new()}
    }

    /// Create a new contra transaction, requires you to know all of its changes at construction.
    pub fn new_contra() -> Mutation {
        Mutation {contra: true, changes: Vec::new()}
    }

    /// Check if this is a contra mutation, i.e. a mutation designed to undo a normal mutation.
    /// This function should not need to be called, ever.
    // pub fn is_contra(&self) -> bool { self.contra }


    /// Designed to help prevent coding errors by making sure contra mutations are not getting mixed
    /// in with the main mutations.
    #[inline]
    pub fn assert_contra(&self) { assert!(self.contra) }

    /// Designed to help prevent coding errors by making sure contra mutations are not getting mixed
    /// in with the main mutations.
    #[inline]
    pub fn assert_not_contra(&self) { assert!(!self.contra) }

    /// Will merge another mutation into this one.
    pub fn merge(&mut self, mut other: Mutation) {
        assert!(!self.contra && !other.contra); //Could be a bug if merging contras
        self.changes.append(&mut other.changes);
    }

    /// Will merge another mutation into this one.
    pub fn merge_clone(&mut self, other: &Mutation) {
        assert!(!self.contra && !other.contra); //Could be a bug if merging contras
        self.changes.extend_from_slice(&other.changes)
    }

    /// Calculate the encoded size of this mutation in bytes.
    pub fn calculate_size(&self) -> usize {
        1 + 8 + // contra + changes count
        self.changes.iter().fold(0, |total, c| total + c.calculate_size())
    } 
}



#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JChange {
    Admin { key: JBin, value: Option<JBin> },
    BlockReward { id: JU160, proof: JBin },
    Event { id: PlotID, tick: u64, event: JPlotEvent },
    NewValidator { pub_key: JBin },
    Slash { id: JU160, amount: u64, proof: JBin }
}

impl From<Change> for JChange {
    fn from(c: Change) -> JChange {
        match c {
            Change::Admin{key, value} => JChange::Admin{key: key.into(), value: value.map(Into::into)},
            Change::BlockReward{id, proof} => JChange::BlockReward{id: id.into(), proof: proof.into()},
            Change::Event{id, tick, event} => JChange::Event{id, tick, event: event.into()},
            Change::NewValidator{pub_key} => JChange::NewValidator{pub_key: pub_key.into()},
            Change::Slash{id, amount, proof} => JChange::Slash{id: id.into(), amount, proof: proof.into()}
        }
    }
}

impl Into<Change> for JChange {
    fn into(self) -> Change {
        match self {
            JChange::Admin{key, value} => Change::Admin{key: key.into(), value: value.map(Into::into)},
            JChange::BlockReward{id, proof} => Change::BlockReward{id: id.into(), proof: proof.into()},
            JChange::Event{id, tick, event} => Change::Event{id, tick, event: event.into()},
            JChange::NewValidator{pub_key} => Change::NewValidator{pub_key: pub_key.into()},
            JChange::Slash{id, amount, proof} => Change::Slash{id: id.into(), amount, proof: proof.into()}
        }
    }
}


#[derive(Serialize, Deserialize)]
pub struct JMutation {
    contra: bool,
    changes: Vec<JChange>
}

impl From<Mutation> for JMutation {
    fn from(m: Mutation) -> JMutation {
        JMutation{contra: m.contra, changes: m.changes.into_iter().map(Into::into).collect()}
    }
}

impl Into<Mutation> for JMutation {
    fn into(self) -> Mutation {
        Mutation{contra: self.contra, changes: self.changes.into_iter().map(Into::into).collect()}
    }
}