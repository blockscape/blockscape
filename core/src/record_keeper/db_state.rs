use bin::{Bin, AsBin};
use bincode::{deserialize, serialize, Bounded, Infinite};
use primitives::{U160, U256, RawEvents, RawEvent, event, BoundingBox};
use super::database::{self, Database, HeadRef, UpIter, DownIter};
use super::{Error, PlotID, DBDiff, key::*};
use super::error::map_not_found;
use serde::de::DeserializeOwned;
use rocksdb::WriteBatch;
use std::collections::{BTreeSet, HashSet};

/// A snapshot of the network state at a given point in time. This builds on a reference to the
/// database with a diff to allow being at a point in time without modifying the DB. This will hold
/// a read lock on the database, so it is important to hold it for only as long as needed.
pub struct DBState<'a> {
    db: &'a dyn Database,
    pub diff: DBDiff,
    head: HeadRef
}

impl<'db> DBState<'db> {
    /// Create a new DB snapshot given a reference to the DB and a DB difference (or changes which
    /// have been or are to be made).
    pub fn new(db: &'db dyn Database, diff: DBDiff) -> DBState<'db> {
        let block = db.get_current_block_hash();
        let height = db.get_current_block_height();
        DBState {
            db,
            diff,
            head: HeadRef{block, height}
        }
    }

    pub fn get_obj<T: DeserializeOwned>(&self, key: Key) -> Result<T, Error> {
        let raw = self._get(key)?;
        Ok(deserialize(&raw)?)
    }

//    /// Get the public key of a validator given their ID.
//    /// See `get_validator_key` in `Database`
//    pub fn get_validator_key(&self, id: U160) -> Result<Bin, Error> {
//        self.get(NetworkEntry::ValidatorKey(id).into())
//    }
//
//    /// Get the reputation of a validator given their ID.
//    /// See `get_validator_rep` in `Database`
//    pub fn get_validator_stake(&self, id: U160) -> Result<u64, Error> {
//        let key = NetworkEntry::ValidatorStake(id).into();
//        let raw = self.get(key)?;
//        Ok(bincode::deserialize::<u64>(&raw)?)
//    }

    pub fn compile(self) -> Result<WriteBatch, Error> {
        let mut wb = WriteBatch::default();
        for (key, value) in self.diff.get_value_changes() {
            if let Some(v) = value {
                wb.put(&key.as_bin(), v)?;
            } else {
                wb.delete(&key.as_bin())?;
            }
        }

        // need to hit each plot just once
        for (plot, remove, add) in self.diff.get_event_changes() {
            // set of the affected tick buckets
            let tick_buckets = {
                let mut tb = BTreeSet::new();

                if let Some(add) = add {
                    for (t, _) in add.iter() {
                        tb.insert(*t / database::PLOT_EVENT_BUCKET_SIZE);
                    }
                }

                if let Some(remove) = remove {
                    for (t, _) in remove.iter() {
                        tb.insert(*t / database::PLOT_EVENT_BUCKET_SIZE);
                    }
                }

                tb
            };

            // initialize any new buckets required so they can be overwritten if need be.


            for tb in tick_buckets.into_iter().rev() {
                let mut current =
                    self.db._get_plot_event_bucket(
                        plot,
                        tb * database::PLOT_EVENT_BUCKET_SIZE
                    )?.unwrap_or_else(RawEvents::new);

                // TODO: do we need to init buckets? Perhaps start from greatest tick bucket first so we can overwrite the initializations.
                unimplemented!()
//                event::add_event
            }
        }

        unimplemented!()
    }
}

impl<'db> Database for DBState<'db> {
    /// Check if there are no entries in either the database or in the additions to it.
    #[inline]
    fn is_empty(&self) -> bool {
        self.db.is_empty() && self.diff.is_empty()
    }

    /// Retrieve a value first from the diff if it has been defined, and then from the database if
    /// not. This will return a NotFound Error if the value is not in the database or if it has been
    /// 'deleted' in the diff.
    fn _get(&self, key: Key) -> Result<Vec<u8>, Error> {
        if let Key::Network(NetworkEntry::Plot(..)) = key {
            unimplemented!()
        }

        if let Some(v) = self.diff.get_value(&key) {
            Ok(v.clone())
        } else if self.diff.is_value_deleted(&key) {
            Err(Error::NotFound(key))
        } else {
            self.db._get(key)
        }
    }

    /// Write a key to the database by writing it to the diff. It will also remove it from the
    /// deletion list of it is there.
    fn _put(&mut self, key: Key, data: &[u8]) -> Result<(), Error> {
        if let Key::Network(NetworkEntry::Plot(..)) = key {
            unimplemented!()
        }

        self.diff.set_value(key, data.into());
        Ok(())
    }

    /// Delete a key from the database by marking it to be deleted in the diff. It will also remove
    /// it from the list of new values if it has been set there.
    fn _delete(&mut self, key: Key) -> Result<(), Error> {
        self.diff.delete_value(key);
        Ok(())
    }

    fn apply(&mut self, wb: WriteBatch) -> Result<(), Error> {
        unimplemented!("Cannot apply a write batch to a DBState object.")
    }

    fn _get_plot_event_bucket(&self, plot_id: PlotID, tick: u64) -> Result<Option<RawEvents>, Error> {
        unimplemented!("Cannot get an event bucket from a DBState object.")
    }

    fn _put_plot_event_bucket(&mut self, plot_id: PlotID, tick: u64, event_list: &RawEvents) -> Result<(), Error> {
        unimplemented!("Cannot set an event bucket in a DBState object.")
    }

    fn _init_event_buckets(&mut self, plot_id: PlotID, before_tick: u64) -> Result<(), Error> {
        unimplemented!("Cannot initialize events buckets for a DBState object.")
    }

    /// Returns a map of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `from_tick` simply allows additional filtering, e.g. if
    /// you set `from_tick` to 0, you would not get all events unless the oldest events have not
    /// yet been removed from the cache.
    fn get_plot_events(&self, plot_id: PlotID, from_tick: u64) -> Result<RawEvents, Error> {
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

    /// Add a new event to the specified plot.
    fn _add_event(&mut self, plot_id: PlotID, tick: u64, event: &RawEvent) -> Result<(), Error> {
        self.diff.add_event(plot_id, tick, event.clone());
        Ok(())
    }

    /// Remove an event from a plot. Should only be used when undoing a mutation.
    fn _remove_event(&mut self, plot_id: PlotID, tick: u64, event: &RawEvent) -> Result<(), Error> {
        self.diff.remove_event(plot_id, tick, event.clone());
        Ok(())
    }

    fn iter_up<'a>(&'a self, start_height: u64) -> UpIter<'a> {
        UpIter::new(self, start_height)
    }

    fn iter_down<'a>(&'a self, start_block: U256) -> DownIter<'a> {
        DownIter::new(self, start_block)
    }

    fn get_current_block_hash(&self) -> U256 {
        self.head.block
    }

    fn get_current_block_height(&self) -> u64 {
        self.head.height
    }

    fn _update_current_block(&mut self, hash: U256, height: Option<u64>) -> Result<(), Error> {
        let h = { // set the height value if it does not exist
            if let Some(h) = height { h }
                else { self.get_block_height(hash)? }
        };

        let href = HeadRef{height: h, block: hash};
        self.head = href.clone();
        self._put(CacheEntry::CurrentHead.into(), &serialize(&href, Bounded(40)).unwrap())
    }
}