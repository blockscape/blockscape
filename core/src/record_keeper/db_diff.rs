use bin::{AsBin, Bin};
use bincode;
use hash::hash_pub_key;
use primitives::{Change, Mutation, U256, U160, RawEvent, RawEvents, event, Coord, BoundingBox};
use record_keeper::PlotEvent;
use std::collections::{HashMap, HashSet};
use super::{PlotID, DBState, key::*};
use super::database::{self, Database};


/// A set of changes which define the difference from a given state to another through walking the
/// blockchain from one point to another. This should be used to compile a list of changes to the
/// database state without having to write to the same place in the DB multiple times.
///
/// Ultimately, this should be used as a base for making multiple changes to the DB at a time
/// because pending changes can be read as if they are already part of the database and a final
/// WriteBatch can be constructed which can then be applied to the database itself. It can also
/// just be used to view what the Database would be like after a series of changes are made without
/// ever changing the Database.
///
/// The way deletion works, is if something is added and then deleted, it does not just remove it
/// form the changes to make, but actually denotes that it will need to be deleted from the database
/// as well. If something which was denoted as being deleted is then set to a value, it will remove
/// it from the list of things to delete and add it to the list of values to set.
///
/// *Warning*: Do not manually add or remove events, use the functions dedicated to them. This is
/// because of how the tick buckets work and prevents
#[derive(Debug)]
pub struct DBDiff {
    /// Only pay attention to changes specified within this set of filters for kv sets. Works independently of plot events
    filters: Option<Vec<Bin>>,
    /// Bounding box to filter which plots should be retained by this NetDiff. Works independently of KV statuses
    bounds: Option<BoundingBox>,
    /// New key-value sets to be added (or overwritten). Keys do not include the Network postfix.
    new_values: HashMap<Key, Bin>,
    /// Keys which are to be removed from the DB
    del_values: HashSet<Key>,
    /// Events which need to be added to plots
    new_events: HashMap<PlotID, RawEvents>,
    /// Events which need to be removed from plots
    del_events: HashMap<PlotID, RawEvents>
}

impl DBDiff {
    pub fn new(mut filters: Option<Vec<Bin>>, bounds: Option<BoundingBox>) -> DBDiff {

        match filters.as_mut() {
            Some(ref mut f) => f.sort_unstable(),
            None => {}
        };

        DBDiff {
            filters, bounds,
            new_values: HashMap::new(),
            del_values: HashSet::new(),
            new_events: HashMap::new(),
            del_events: HashMap::new()
        }
    }

    pub fn into_state<'a, DB: Database>(self, db: &'a DB) -> DBState<'a> {
        DBState::new(db, self)
    }

    /// Check if there are no new values or events.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.new_values.is_empty() && self.new_events.is_empty()
    }

    /// Add an event to the appropriate plot
    pub fn add_event(&mut self, id: PlotID, tick: u64, event: RawEvent) {
        // do bounding box filter
        if self.bounds.is_some() &&
            !self.bounds.unwrap().contains(id) {
            return;
        }

        //if it was in removed events, then we don't need to add it
        if !Self::remove(&mut self.del_events, id, tick, &event) {
            Self::add(&mut self.new_events, id, tick, event);
        }
    }

    /// Remove an event from the appropriate plot (or mark it to be removed).
    pub fn remove_event(&mut self, id: PlotID, tick: u64, event: RawEvent) {
        // do bounding box filter
        if self.bounds.is_some() &&
            !self.bounds.unwrap().contains(id) {
            return;
        }

        //if it was in new events events, then we don't need to add it be removed
        if !Self::remove(&mut self.new_events, id, tick, &event) {
            Self::add(&mut self.del_events, id, tick, event)
        }
    }

    /// Mark a value to be updated at a given key.
    pub fn set_value(&mut self, key: Key, value: Bin) {
        if let Some(ref f) = self.filters {
            let pos = f.binary_search(&key.as_bin());

            if pos.is_err() {
                return;
            }
        }

        self.del_values.remove(&key);
        self.new_values.insert(key, value);
    }

    /// Mark a key and its value to be removed from the state.
    pub fn delete_value(&mut self, key: Key) {
        self.new_values.remove(&key);
        self.del_values.insert(key);
    }

    /// Retrieve a list of new events for a given plot.
    pub fn get_new_events(&self, plot: PlotID) -> Option<&RawEvents> {
        self.new_events.get(&plot)
    }

    /// Retrieves a list of events to be removed from a given plot.
    pub fn get_removed_events(&self, plot: PlotID) -> Option<&RawEvents> {
        self.del_events.get(&plot)
    }

    /// Retrieve the value if any changes have been specified to it. Will return none if no changes
    /// are recorded or if it is to be deleted.
    pub fn get_value(&self, key: &Key) -> Option<&Bin> {
        self.new_values.get(key)
    }

    /// Returns whether or not a given value is marked for deletion.
    pub fn is_value_deleted(&self, key: &Key) -> bool {
        self.del_values.contains(key)
    }

    /// Check if an event has been marked for removal from its associated plots.
    pub fn is_event_removed(&self, plot: PlotID, tick: u64, event: &RawEvent) -> bool {
        if let Some(plot) = self.del_events.get(&plot) {
            if let Some(events) = plot.get(&tick) {
                events.contains(event)
            } else { false }
        } else { false }
    }

    /// Get an iterator over each Plot we have information on and give a list of all things to
    /// remove for it and all things to add to it. See `EventDiffIter`.
    pub fn get_event_changes<'a>(&'a self) -> EventDiffIter {
        let keys = {
            let added: HashSet<_> = self.new_events.keys().cloned().collect();
            let removed: HashSet<_> = self.del_events.keys().cloned().collect();
            added.union(&removed).cloned().collect::<Vec<_>>()
        };

        EventDiffIter(self, keys.into_iter())
    }

    /// Get an iterator over each key we have information on and return if it is deleted or the new
    /// value it should be set to. See `ValueDiffIter`.
    pub fn get_value_changes<'a>(&'a self) -> ValueDiffIter {
        let keys: Vec<&'a Key> = {
            let added: HashSet<_> = self.new_values.keys().collect();
            let removed: HashSet<_> = self.del_values.iter().collect();
            added.union(&removed).cloned().collect()
        };

        ValueDiffIter(self, keys.into_iter())
    }


    /// Attempt to remove an event from list and return whether it was was there or not.
    fn remove(plots: &mut HashMap<PlotID, RawEvents>, id: PlotID, tick: u64, event: &RawEvent) -> bool {
        if let Some(plot) = plots.get_mut(&id) {
            event::remove_event(plot, tick, event)
        } else { false } // did not remove because plot is not listed)
    }

    fn add(plots: &mut HashMap<PlotID, RawEvents>, id: PlotID, tick: u64, event: RawEvent) {
        // check if we need to create a new entry (if not go ahead and append it)
        if let Some(plot) = plots.get_mut(&id) {
            event::add_event(plot, tick, event);
            return;
        }

        // insert a new entry
        let mut plot = RawEvents::new();
        event::add_event(&mut plot, tick, event);
        plots.insert(id, plot);
    }
}

impl<'a> From<DBState<'a>> for DBDiff {
    fn from(s: DBState) -> Self {
        s.diff
    }
}

impl Default for DBDiff {
    fn default() -> Self {
        DBDiff::new(None, None)
    }
}



use std::vec::IntoIter as VecIntoIter;

// TODO: rewrite to not use a vec of keys
/// Iterate over all plots we have event changes to make to. The first value is the key, the next is
/// the list of events to remove, and finally it has the list of new events,
pub struct EventDiffIter<'a>(&'a DBDiff, VecIntoIter<PlotID>);
impl<'a> Iterator for EventDiffIter<'a> {
    type Item = (PlotID, Option<&'a RawEvents>, Option<&'a RawEvents>);

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|k| (k, self.0.get_removed_events(k), self.0.get_new_events(k)) )
    }
}

// TODO: rewrite to not use a vec of keys
/// Iterate over all values we have changes recorded for. The first part of the Item is the key, and
/// the second part is the value, if the value is None, then the key should be deleted from the DB.
pub struct ValueDiffIter<'a>(&'a DBDiff, VecIntoIter<&'a Key>);
impl<'a> Iterator for ValueDiffIter<'a> {
    type Item = (&'a Key, Option<&'a Bin>);

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|k| {
            (k, self.0.get_value(k))
        })
    }
}