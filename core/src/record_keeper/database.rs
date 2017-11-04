use bincode;
use env;
use primitives::{Event, Events, U256, Mutation, Change};
use rocksdb::{DB, WriteBatch, Options};
use rocksdb::Error as RocksDBError;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::RwLock;
use super::error::*;
use super::{Storable, PlotEvent, PlotID};

pub const BLOCKCHAIN_POSTFIX: &[u8] = b"b";
pub const CACHE_POSTFIX: &[u8] = b"c";
pub const NETWORK_POSTFIX: &[u8] = b"n";

pub const PLOT_PREFIX: &[u8] = b"PLOT";

#[inline]
fn extend_vec(mut k: Vec<u8>, post: &[u8]) -> Vec<u8> {
    k.extend_from_slice(post); k
}

fn plot_key(id: &PlotID) -> Vec<u8> {
    let mut k = Vec::from(PLOT_PREFIX);
    k.append(&mut id.bytes());
    k.extend_from_slice(NETWORK_POSTFIX); k
}


/// This is a wrapper around a RocksDB instance to provide the access and modifications needed for
/// our system.
/// Please note that there are three distinct "regions" of the database:
/// - The **blockcahin state** stores the blocks and transactions by their hashes.
/// - The **game state** stores plots and their associated data, possibly other things as well.
/// - The **network state** stores the results of transactions being applied, things like who is a
///   valid miner, reputations, checkpoints/snapshots, and more.
/// To keep these regions separate, postfixes are appended before accessing the database, this will
/// prevent conflicts between the different regions even if they are using non-secure hashing
/// methods.
///
/// TODO: Remove events older than we allow for a fork from network state
pub struct Database {
    db: DB
}


impl Database {
    /// Create a new Database from a RocksDB instance
    pub fn new(db: DB) -> Database {
        Database{ db }
    }

    /// Open the RocksDB database based on the environment or by the given path. Construct a new
    /// Database by opening an existing one or creating a new database if the one specified does not
    /// exist. If no path is provided, it will open the database in the directory
    /// `env::get_storage_dir()`.
    /// # Warning
    /// Any database which is opened, is assumed to contain data in a certain way, any outside
    /// modifications can cause undefined behavior.
    pub fn open(path: Option<PathBuf>) -> Result<Database, RocksDBError> {
        let mut options = Options::default();
        options.create_if_missing(true);

        let dir = match path {
            Some(p) => p,
            None => {
                let mut d = env::get_storage_dir().unwrap();
                d.push("db"); d
            }
        };

        Ok(
            DB::open_default(dir)
            .map(|db| Self::new(db))?
        )
    }

    /// Mutate the stored **network state** and return a contra mutation to be able to undo what was
    /// done. Note that changes to either blockchain state or gamestate must occur through other
    /// functions.
    pub fn mutate(&mut self, mutation: &Mutation) -> Result<Mutation, Error> {
        mutation.assert_not_contra();
        let mut contra = Mutation::new_contra();

        for change in &mutation.changes { match change {
            &Change::SetValue{ref key, ref value, ..} => {
                let db_key = extend_vec(key.clone(), NETWORK_POSTFIX);
                
                contra.changes.push(Change::SetValue {
                    key: key.clone(),
                    value: self.db.get(&db_key)?.map(|v| v.to_vec()), // Option<Vec<u8>>
                    supp: None
                });

                if let Some(ref v) = *value {
                    self.db.put(&db_key, v)?;
                } else {  // delete key
                    if self.db.delete(&db_key).is_err() {
                        warn!("Unable to delete a key in the network state. The key may not have \
                        existed, or there could be a problem with the database.");
                    }
                }
            },
            &Change::AddEvent{id, tick, ref event, ..} => {
                let db_key = plot_key(&id);

                let mut events: Events<PlotEvent> = self.db.get(&db_key)?.map_or(
                    Events::new(), //if not found, we need to create the data structure
                    |v| bincode::deserialize(&v).unwrap()
                );


                if !{ // get mut ref and append the event.
                    if let Some(ref mut list) = events.get_mut(&tick)
                    { list.push(event.clone()); true } // TODO: Do we need to handle this: if list.contains(event)?
                    else { false }
                }{ // no events registered at this tick, create new list and insert into events.
                    let mut list = Vec::new();
                    list.push(event.clone());
                    events.insert(tick, list);
                }

                let raw_events = bincode::serialize(&events, bincode::Infinite).unwrap();
                self.db.put(&db_key, &raw_events);

                contra.changes.push(Change::AddEvent{id, tick, event: event.clone(), supp: None});
            }
        }}

        contra.changes.reverse(); // contra goes in reverse of original actions
        Ok(contra)
    }

    /// Consumes a contra mutation to undo changes made by the corresponding mutation to the
    /// network state.
    pub fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), Error> {
        mutation.assert_contra();

        for change in mutation.changes { match change {
            Change::SetValue{key, value, ..} => {
                let db_key = extend_vec(key, NETWORK_POSTFIX);

                if let Some(v) = value {
                    self.db.put(&db_key, &v)?;
                } else { // delete key
                    if self.db.delete(&db_key).is_err() {
                        warn!("Unable to delete a key in the network state! The key may not have \
                        existed, or there could be a problem with the database.");
                    }
                }
            },
            Change::AddEvent{id, tick, event, ..} => {
                let db_key = plot_key(&id);

                if let Some(raw_events) = self.db.get(&db_key)? {
                    let mut events: Events<PlotEvent> = bincode::deserialize(&raw_events).unwrap();
                    if let Some(ref mut list) = events.get_mut(&tick) {
                        list.retain(|e| *e != event);
                    } else {
                        warn!("Unable to remove event because it does not exist! The network state \
                               may be desynchronized.");
                        continue;
                    }
                    
                    let raw_events = bincode::serialize(&events, bincode::Infinite).unwrap();
                    self.db.put(&db_key, &raw_events)?;
                } else {
                   warn!("Unable to remove event because it does not exist! The network state \
                              may be desynchronized.");
                }
            }
        }}

        Ok(())
    }

    /// Retrieve raw data from the database. Use this for non-storable types (mostly network stuff).
    pub fn get_raw_data(&self, key: &[u8], postfix: &'static [u8]) -> Result<Vec<u8>, Error> {
        let key = {
            let mut k = Vec::from(key);
            k.extend_from_slice(postfix); k
        };

        self.db.get(&key)?
            .map(|d| d.to_vec())
            .ok_or(Error::NotFound(postfix, Vec::from(key)))
    }

    /// Put raw data into the database. Should have no uses outside this class.
    pub fn put_raw_data(&mut self, key: &[u8], data: &[u8], postfix: &'static [u8]) -> Result<(), Error> {
        let key = {
            let mut k = Vec::from(key);
            k.extend_from_slice(postfix); k
        };

        Ok(self.db.put(&key, &data)?)
    }
}