use primitives::{Change, Mutation, U256};
use bin::Bin;
use super::{PlotEvent, PlotEvents, PlotID, events};
use std::collections::{HashMap, HashSet};

/// A set of changes which define the difference from a given network state to another though
/// walking the blockchain from one point to another. This should be used to compile a list of
/// changes to the network state without having to write to the same place in the DB multiple times.
/// This is designed to be like a diff, so if an event is added but it had been marked as deleted,
/// then it will simply remove it from the list of deleted under the assumption that the net change
/// should be zero.
pub struct NetDiff {
    /// The initial block this is changing from
    pub from: U256,
    /// The block all these changes lead to (if applied to the initial block)
    pub to: U256,
    /// New key-value sets to be added (or overwritten). Keys do not include the Network postfix.
    values: HashMap<Bin, Bin>,
    /// Keys which are to be removed from the DB
    delete: HashSet<Bin>,
    /// Events which need to be added to plots
    new_events: HashMap<PlotID, PlotEvents>,
    /// Events which need to be removed from plots
    removed_events: HashMap<PlotID, PlotEvents>
}

impl NetDiff {
    pub fn new(from: U256, to: U256) -> NetDiff {
        NetDiff {
            from, to,
            values: HashMap::new(),
            delete: HashSet::new(),
            new_events: HashMap::new(),
            removed_events: HashMap::new()
        }
    }

    /// Add an event to the appropriate plot
    pub fn add_event(&mut self, id: PlotID, tick: u64, event: PlotEvent) {
        //if it was in removed events, then we don't need to add it
        if !Self::remove(&mut self.removed_events, id, tick, &event) {
            Self::add(&mut self.new_events, id, tick, event);
        }
    }

    /// Remove an event from the appropriate plot (or mark it to be removed).
    pub fn remove_event(&mut self, id: PlotID, tick: u64, event: PlotEvent) {
        //if it was in new events events, then we don't need to add it be removed
        if !Self::remove(&mut self.new_events, id, tick, &event) {
            Self::add(&mut self.removed_events, id, tick, event)
        }
    }

    /// Mark a value to be updated at a given key.
    pub fn set_value(&mut self, key: Bin, value: Bin) {
        self.delete.remove(&key);
        self.values.insert(key, value);
    }

    /// Mark a key and its value to be removed from the state.
    pub fn delete_value(&mut self, key: Bin) {
        self.values.remove(&key);
        self.delete.insert(key);
    }

    /// Apply all the changes in a mutation to this diff.
    pub fn apply_mutation(&mut self, m: Mutation) {
        m.assert_not_contra();
        for change in m.changes { match change {
            Change::SetValue{key, value, ..} => {
                if let Some(v) = value { self.set_value(key, v); }
                else { self.delete_value(key); }
            },
            Change::AddEvent{id, tick, event, ..} => {
                self.add_event(id, tick, event);
            }
        }}
    }

    /// Apply all the changes in a contra-mutation to this diff.
    pub fn apply_contra(&mut self, m: Mutation) {
        m.assert_contra();
        for change in m.changes { match change {
            Change::SetValue{key, value, ..} => {
                if let Some(v) = value { self.set_value(key, v); }
                else { self.delete_value(key) }
            },
            Change::AddEvent{id, tick, event, ..} => {
                self.remove_event(id, tick, event);
            }
        }}
    }

    /// Retrieve a list of new events for a given plot.
    pub fn get_new_events(&self, plot: PlotID) -> Option<&PlotEvents> {
        self.new_events.get(&plot)
    }

    /// Retrieves a list of events to be removed from a given plot.
    pub fn get_removed_events(&self, plot: PlotID) -> Option<&PlotEvents> {
        self.removed_events.get(&plot)
    }

    /// Retrieve the value if any changes have been specified to it. Will return none if no changes
    /// are recorded or if it is to be deleted.
    pub fn get_value(&self, key: &Bin) -> Option<&Bin> {
        self.values.get(key)
    }

    /// Returns whether or not a given value is marked for deletion.
    pub fn is_value_deleted(&self, key: &Bin) -> bool {
        self.delete.contains(key)
    }

    /// Check if an event has been marked for removal from its associated plots.
    pub fn is_event_removed(&self, plot: PlotID, tick: u64, event: &PlotEvent) -> bool {
        if let Some(plot) = self.removed_events.get(&plot) {
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
            let removed: HashSet<_> = self.removed_events.keys().cloned().collect();
            added.union(&removed).cloned().collect::<Vec<_>>()
        };
        
        EventDiffIter(self, keys.into_iter())
    }

    /// Get an iterator over each key we have information on and return if it is deleted or the new
    /// value it should be set to. See `ValueDiffIter`.
    pub fn get_value_changes<'a>(&'a self) -> ValueDiffIter {
        let keys: Vec<&'a Bin> = {
            let added: HashSet<_> = self.values.keys().collect();
            let removed: HashSet<_> = self.delete.iter().collect();
            added.union(&removed).cloned().collect()
        };

        ValueDiffIter(self, keys.into_iter())
    }


    /// Attempt to remove an event from list and return whether it was was there or not.
    fn remove(plots: &mut HashMap<PlotID, PlotEvents>, id: PlotID, tick: u64, event: &PlotEvent) -> bool {
        if let Some(plot) = plots.get_mut(&id) {
            events::remove_event(plot, tick, event)
        } else { false } // did not remove because plot is not listed)
    }

    fn add(plots: &mut HashMap<PlotID, PlotEvents>, id: PlotID, tick: u64, event: PlotEvent) {
        // check if we need to create a new entry (if not go ahead and append it)
        if let Some(plot) = plots.get_mut(&id) {
            events::add_event(plot, tick, event);
            return;
        }

        // insert a new entry
        let mut plot = PlotEvents::new();
        events::add_event(&mut plot, tick, event);
        plots.insert(id, plot);
    }
}

use std::vec::IntoIter as VecIntoIter;

/// Iterate over all plots we have event changes to make to. The first value is the key, the next is
/// the list of events to remove, and finally it has the list of new events,
pub struct EventDiffIter<'a>(&'a NetDiff, VecIntoIter<PlotID>);
impl<'a> Iterator for EventDiffIter<'a> {
    type Item = (PlotID, Option<&'a PlotEvents>, Option<&'a PlotEvents>);

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|k| (k, self.0.get_removed_events(k), self.0.get_new_events(k)) )
    }
}

/// Iterate over all values we have changes recorded for. The first part of the Item is the key, and
/// the second part is the value, if the value is None, then the key should be deleted from the DB.
pub struct ValueDiffIter<'a>(&'a NetDiff, VecIntoIter<&'a Bin>);
impl<'a> Iterator for ValueDiffIter<'a> {
    type Item = (&'a Bin, Option<&'a Bin>);

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|k| {
            (k, self.0.get_value(k))
        })
    }
}