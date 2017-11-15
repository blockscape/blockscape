use bincode;
use bytes::{BigEndian, ByteOrder};
use env;
use primitives::{U256, Mutation, Change, Block, BlockHeader, Txn};
use rocksdb::{DB, Options};
use rocksdb::Error as RocksDBError;
use std::collections::HashSet;
use std::path::PathBuf;
use super::{Storable, PlotEvent, PlotEvents, events, PlotID};
use super::error::*;

pub const BLOCKCHAIN_POSTFIX: &[u8] = b"b";
pub const CACHE_POSTFIX: &[u8] = b"c";
pub const NETWORK_POSTFIX: &[u8] = b"n";

pub const PLOT_PREFIX: &[u8] = b"PLT";
pub const HEIGHT_PREFIX: &[u8] = b"HGT";

#[inline]
fn extend_vec(mut k: Vec<u8>, post: &[u8]) -> Vec<u8> {
    k.extend_from_slice(post); k
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

    /// Retrieve and deserialize data from the database. This will return an error if the database
    /// has an issue, if the data cannot be deserialized or if the object is not present in the
    /// database. Note that `instance_id` should be the object's ID/key which would normally be
    /// returned from calling `storable.instance_id()`.
    pub fn get<S: Storable>(&self, instance_id: &[u8], postfix: &'static [u8]) -> Result<S, Error> {
        let key = {
            let mut k = Vec::from(S::global_id());
            k.extend_from_slice(instance_id); k
        };

        let raw = self.get_raw_data(&key, postfix)?;
        Ok(bincode::deserialize::<S>(&raw)?)
    }

    /// Serialize and store data in the database. This will return an error if the database has an
    /// issue.
    pub fn put<S: Storable>(&mut self, obj: &S, postfix: &'static [u8]) -> Result<(), Error> {
        let value = bincode::serialize(obj, bincode::Infinite)
            .expect("Error serializing game data.");
        self.put_raw_data(&obj.key(), &value, postfix)
    }

    pub fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        let id = hash.to_vec();
        self.get::<BlockHeader>(&id, BLOCKCHAIN_POSTFIX)
    }

    pub fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        // Blocks are stored with their header separate from the transaction body, so get the header
        // first to find the merkle_root, and then get the list of transactions and piece them
        // together.
        let header = self.get_block_header(hash)?;
        self.complete_block(header)
    }

    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let merkle_root = header.merkle_root.to_vec();
        let raw_txns = self.get_raw_data(&merkle_root, BLOCKCHAIN_POSTFIX)?;
        Ok(Block::deserialize(header, &raw_txns)?)
    }

    pub fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let id = hash.to_vec();
        self.get::<Txn>(&id, BLOCKCHAIN_POSTFIX)
    }

    /// Add a block the set of known blocks at a given height.
    pub fn add_block_to_height(&mut self, height: u64, block: U256) -> Result<(), Error> {
        let key = Self::get_block_height_key(height);
        let res = self.get_raw_data(&key, CACHE_POSTFIX);
        
        let mut height_vals: HashSet<U256> = {
            match res {
                Ok(raw) => bincode::deserialize(&raw)?,
                Err(e) => match e {
                    Error::NotFound(..) => HashSet::new(),
                    _ => return Err(e)
                }
            }
        };

        height_vals.insert(block);
        let raw = bincode::serialize(&height_vals, bincode::Infinite)?;
        self.put_raw_data(&key, &raw, CACHE_POSTFIX)
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    pub fn get_blocks_of_height(&self, height: u64) -> Result<HashSet<U256>, Error> {
        let key = Database::get_block_height_key(height);
        let res = self.get_raw_data(&key, CACHE_POSTFIX);
        match res {
            Ok(raw) => { // found something, deserialize
                Ok(bincode::deserialize::<HashSet<U256>>(&raw)?)
            },
            Err(e) => match e {
                Error::NotFound(..) => // nothing known to us, so emptyset
                    Ok(HashSet::new()),
                _ => Err(e) // some sort of database error
            }
        }
    }

    /// Calculate the height of a given block. It will follow the path until it finds the genesis
    /// block which is denoted by having a previous block reference of 0.
    pub fn get_block_height(&self, hash: &U256) -> Result<u64, Error> {
        let mut h: u64 = 0;
        let mut block = self.get_block_header(hash)?;

        while !block.prev.is_zero() {  // while we have not reached genesis...
            h += 1;
            block = self.get_block_header(&block.prev)?;
        } Ok(h)
    }

    /// Get the key value for the height cache in the database. (Without the cache postfix).
    pub fn get_block_height_key(height: u64) -> Vec<u8> {
        let mut buf = [0u8; 8];
        BigEndian::write_u64(&mut buf, height);
        let mut k = Vec::from(HEIGHT_PREFIX);
        k.extend_from_slice(&buf); k
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
                self.add_plot_event(id, tick, event)?;
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
                let db_key = Self::plot_key(&id);

                if let Some(raw_events) = self.db.get(&db_key)? {
                    let mut events: PlotEvents = bincode::deserialize(&raw_events).unwrap();
                    if !events::remove_event(&mut events, tick, &event) {
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

    /// Add a new event to a plot
    pub fn add_plot_event(&mut self, plot_id: PlotID, tick: u64, event: &PlotEvent) -> Result<(), Error> {
        let db_key = Self::plot_key(&plot_id);

        let mut events: PlotEvents = self.db.get(&db_key)?.map_or(
            PlotEvents::new(), //if not found, we need to create the data structure
            |v| bincode::deserialize(&v).unwrap()
        );

        events::add_event(&mut events, tick, event.clone());

        let raw_events = bincode::serialize(&events, bincode::Infinite).unwrap();
        self.db.put(&db_key, &raw_events)?;
        Ok(())
    }

    /// Returns a map of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `after_tick` simply allows additional filtering, e.g. if
    /// you set `after_tick` to 0, you would not get all events unless the oldest events have not
    /// yet been removed from the cache.
    pub fn get_plot_events(&self, plot_id: PlotID, after_tick: u64) -> Result<PlotEvents, Error> {
        let db_key = Self::plot_key(&plot_id);
        Ok(self.db.get(&db_key)?.map_or(
            PlotEvents::new(),
            |v| bincode::deserialize(&v).unwrap()
        ))
    }



    pub fn plot_key(id: &PlotID) -> Vec<u8> {
        let mut k = Vec::from(PLOT_PREFIX);
        k.append(&mut id.bytes());
        k.extend_from_slice(NETWORK_POSTFIX); k
    }
}