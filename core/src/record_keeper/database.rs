use bincode;
use env;
use primitives::{Change, Mutation};
use primitives::U256;
use rocksdb::{DB, WriteBatch, Options};
use rocksdb::Error as RocksDBError;
use std::collections::{LinkedList, BTreeSet};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::RwLock;
use super::error::*;
use super::storable::Storable;

pub const BLOCKCHAIN_POSTFIX: &[u8] = b"b";
pub const CACHE_POSTFIX: &[u8] = b"c";
pub const NETWORK_POSTFIX: &[u8] = b"n";

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
        let mut batch = WriteBatch::default();
        let mut del = BTreeSet::new();  // set of keys to be deleted

        for change in &mutation.changes {
            let key = {
                let mut k = change.key.clone();
                k.extend_from_slice(NETWORK_POSTFIX); k
            };
            
            contra.changes.push(Change {
                key: key.clone(),
                value: self.db.get(&key)?.map(|v| v.to_vec()), // Option<Vec<u8>>
                data: None,
            });

            if let Some(ref v) = change.value {
                del.remove(&key);
                batch.put(&key, v).expect("Failure when adding to rocksdb batch.");
            } else {  // delete key
                batch.delete(&key).is_ok(); // ignore error if there is one
                del.insert(key.clone()); // add it to list of things to remove
            }
        }
        self.db.write(batch)?;

        for i in del {
            if self.db.delete(&i).is_err() {
                warn!("Unable to delete a key in network state data. They key may not have \
                       existed, or there could be a problem with the database.");
            }
        }

        contra.changes.reverse();
        Ok(contra)
    }

    /// Consumes a contra mutation to undo changes made by the corresponding mutation to the
    /// network state.
    pub fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), Error> {
        mutation.assert_contra();
        let mut batch = WriteBatch::default();
        let mut del = BTreeSet::new();

        for change in &mutation.changes {
            let key = {
                let mut k = change.key.clone();
                k.extend_from_slice(NETWORK_POSTFIX); k
            };

            if let Some(ref v) = change.value {
                del.remove(&key);
                batch.put(&key, v).expect("Failure when adding to rocksdb batch.");
            } else {  // delete key
                batch.delete(&key).is_ok();
                del.insert(key);
            }
        }

        self.db.write(batch)?;
        for i in del {
            if self.db.delete(&i).is_err() {
                // TODO: should we panic?
                error!("Unable to delete a key in network state when applying a contra mutation!");
            }
        }
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