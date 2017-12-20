use std::hash::{Hash, Hasher};
use record_keeper::{PlotID, PlotEvent};
use std::mem::size_of;

/// A single change to the database, a mutation may be the composite of multiple changes. This is
/// designed as a simple structure which the outer world can use to store the changes which should
/// not know anything about the database. The supplementrary data field is provided for many of the
/// types of changes, it is designed to be information used to verify a transaction, but which does
/// not alter the network state.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Change {
    SetValue { key: Vec<u8>, value: Option<Vec<u8>>, supp: Option<Vec<u8>> },
    AddEvent { id: PlotID, tick: u64, event: PlotEvent, supp: Option<Vec<u8>> }
}

impl PartialEq for Change {
    fn eq(&self, other: &Change) -> bool {
        match (self, other) {
            (&Change::SetValue{key: ref a, ..}, &Change::SetValue{key: ref b, ..}) => a == b,
            (&Change::AddEvent{id: i1, tick: t1, event: ref e1, ..},
             &Change::AddEvent{id: i2, tick: t2, event: ref e2, ..}) => {
                (i1 == i2) && (t1 ==t2) && (e1 == e2) //TODO: will comparing the bits be accurate?
            },
            _ => false
        }
    }
} //TODO: create different Eq which uses event deserialization?

impl Eq for Change {}

impl Hash for Change {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            &Change::SetValue{key: ref k, ..} => k.hash(state),
            &Change::AddEvent{id, tick, ..} => {id.hash(state); tick.hash(state)}
        };
    }
}

impl Change {
    /// Calculate the encoded size of this change in bytes.
    pub fn calculate_size(&self) -> usize {
        8 + match self {
            &Change::SetValue{ref key, ref value, ref supp} => {
                key.len() + 1 +
                if let Some(a) = value.as_ref() { a.len() }
                else { 0 } + 2 +
                if let Some(a) = supp.as_ref() { a.len() }
                else { 0 } + 2
            },
            &Change::AddEvent{ref event, ref supp, ..} => {
                size_of::<PlotID>() + 8 + 
                event.calculate_size() +
                if let Some(a) = supp.as_ref() { a.len() }
                else { 0 } + 2
            }
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

    /// Will merge another mutation into this one. The values from this mutation will be placed
    /// after the other, thus they will have a "higher" priority should there be conflicting txns.
    /// This consume the other mutation and re-use its allocated data.
    pub fn merge(&mut self, mut other: Mutation) {
        assert!(!self.contra && !other.contra); //Could be a bug if merging contras
        
        other.changes.append(&mut self.changes); // empties changes and puts it at the end
        self.changes = other.changes; // now we move all the content of tmp to destination
    }

    /// Will merge another mutation into this one. The values from this mutation will be placed
    /// after the other, thus they will have a "higher" priority should there be conflicting txns.
    /// This will clone data from the incoming mutation.
    pub fn merge_clone(&mut self, other: &Mutation) {
        assert!(!self.contra && !other.contra); //Could be a bug if merging contras
        
        let mut tmp: Vec<Change> = other.changes.clone();
        tmp.append(&mut self.changes);
        self.changes = tmp;
    }

    /// Calculate the encoded size of this mutation in bytes.
    pub fn calculate_size(&self) -> usize {
        1 +  // contra
        self.changes.iter().fold(0, |total, c| total + c.calculate_size())
    } 
}