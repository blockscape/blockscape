use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use primitives::{U256, Event};

/// A single change to the database, a mutation may be the composite of multiple changes. This is
/// designed as a simple structure which the outer world can use to store the changes which should
/// not know anything about the database. The supplementrary data field is provided for many of the
/// types of changes, it is designed to be information used to verify a transaction, but which does
/// not alter the network state.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Change<E: Event> {
    SetValue    { key: U256, value: Option<Vec<u8>>, supp: Option<Vec<u8>> },
    AddEvent { id: u64, tick: u64, event: E, supp: Option<Vec<u8>> }
}

impl<E: Event> PartialEq for Change<E> {
    fn eq(&self, other: &Change<E>) -> bool {
        match (self, other) {
            (&Change::SetValue{key: ref a}, &Change::SetValue{key: ref b}) => a == b,
            (&Change::AddEvent{id: i1, tick: t1, event: ref e1},
             &Change::AddEvent{id: i2, tick: t2, event: ref e2}) => {
                (i1 == i2) && (t1 ==t2) && (e1 == e2)
            },
            _ => false
        }
    }
}

impl<E: Event> Eq for Change<E> {}

impl<E: Event> Hash for Change<E> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}



/// A composition of changes which are to be atomically applied to the database. In a few places,
/// certain actions on contras will fail by assertion, I believe any such error should be a result
/// of a coding mistake and should not need to be determined at runtime.
/// TODO: consider storing a HashSet<Rc<Change>> to save on cloning.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Mutation<E: Event> {
    contra: bool, // true iff it is a normal mutation, else it is a contra mutation.
    pub changes: Vec<Change<E>>
}

impl<E: Event> Mutation<E> {
    /// Create a new, empty mutation.
    pub fn new() -> Mutation<E> {
        Mutation {contra: false, changes: Vec::new()}
    }

    /// Create a new contra transaction, requires you to know all of its changes at construction.
    pub fn new_contra() -> Mutation<E> {
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
    pub fn merge(&mut self, mut other: Mutation<E>) {
        assert!(!self.contra && !other.contra); //Could be a bug if merging contras
        
        other.changes.append(&mut self.changes); // empties changes and puts it at the end
        self.changes = other.changes; // now we move all the content of tmp to destination
    }

    /// Will merge another mutation into this one. The values from this mutation will be placed
    /// after the other, thus they will have a "higher" priority should there be conflicting txns.
    /// This will clone data from both mutations and create a new, independent mutation.
    pub fn merge_clone(&self, other: &Mutation<E>) -> Mutation<E> {
        assert!(!self.contra && !other.contra); //Could be a bug if merging contras
        
        let mut changes = other.changes.clone();
        changes.extend(self.changes.iter().cloned());
        Mutation { contra: false, changes }
    }
}