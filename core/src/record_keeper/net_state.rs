use bin::Bin;
use bincode;
use primitives::{U160, RawEvents, event};
use super::database::Database;
use super::{Error, NetDiff, PlotID};
use super::error::map_not_found;
use super::key::*;
use serde::de::DeserializeOwned;

/// A snapshot of the network state at a given point in time. This builds on a reference to the
/// database with a diff to allow being at a point in time without modifying the DB. This will hold
/// a read lock on the database, so it is important to hold it for only as long as needed.
pub struct NetState<'a> {
    db: &'a Database,
    diff: NetDiff
}

impl<'a> NetState<'a> {
    /// Create a new Network Snapshot given a reference to the db and a network difference.
    pub fn new(db: &'a Database, diff: NetDiff) -> NetState<'a> {
        NetState { db, diff }
    }

    /// Retrieve a value first from the diff if it has been defined, and then from the database if
    /// not. This will return a NotFound Error if the value is not in the database or if it has been
    /// 'deleted' in the diff.
    pub fn get_value(&self, key: Key) -> Result<Bin, Error> {
        if let Some(v) = self.diff.get_value(&key) {
            Ok(v.clone())
        } else if self.diff.is_value_deleted(&key) {
            Err(Error::NotFound(key))
        } else {
            self.db.get_raw_data(key)
        }
    }

    pub fn get_obj<T: DeserializeOwned>(&self, key: Key) -> Result<T, Error> {
        let raw = self.get_value(key)?;
        Ok(bincode::deserialize(&raw)?)
    }

    /// Get the public key of a validator given their ID.
    /// See `get_validator_key` in `Database`
    pub fn get_validator_key(&self, id: U160) -> Result<Bin, Error> {
        self.get_value(NetworkEntry::ValidatorKey(id).into())
    }

    /// Get the reputation of a validator given their ID.
    /// See `get_validator_rep` in `Database`
    pub fn get_validator_rep(&self, id: U160) -> Result<i64, Error> {
        let key = NetworkEntry::ValidatorRep(id).into();
        let raw = self.get_value(key)?;
        Ok(bincode::deserialize::<i64>(&raw)?)
    }

    pub fn get_plot_events(&self, plot_id: PlotID, from_tick: u64) -> Result<RawEvents, Error> {
        let new_events = self.diff.get_new_events(plot_id);
        let removed_events = self.diff.get_removed_events(plot_id);
        
        // get the base events from the DB
        let mut plot_events = map_not_found(
            self.db.get_plot_events(plot_id, from_tick),
            RawEvents::new()
        )?;


        // remove the removed events
        if let Some(removed_e) = removed_events {
            for (&tick, r_event_list) in removed_e.range(from_tick..) {
                // if tick <= from_tick { continue; }
                for ref event in r_event_list {
                    event::remove_event(&mut plot_events, tick, event);
                }
            }
        }

        // add the new events
        if let Some(new_e) = new_events {
            for (&tick, n_event_list) in new_e.range(from_tick..) {
                for ref event in n_event_list {
                    event::add_event(&mut plot_events, tick, (*event).clone());
                }
            }
        }

        Ok(plot_events)
    }
}