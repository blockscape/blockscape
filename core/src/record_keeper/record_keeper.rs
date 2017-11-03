use bincode;
use bytes::{BigEndian, ByteOrder};
use primitives::{Event, RawEvent, Events, EventListener};
use primitives::{U256, U160, Txn, Block, BlockHeader, Mutation};
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet};
use std::collections::LinkedList;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::{Arc, RwLock, Weak};
use super::{MutationRule, MutationRules, Error, Storable, PlotID, PlotEvent};
use super::database::*;

const HEIGHT_PREFIX: &[u8] = b"h";


/// An event regarding the keeping of records, such as the introduction of a new block or shifting
/// state.
///
/// **Note:** notifications will only be sent once the changes to state have been applied unless
/// otherwise stated. This means that if there is a `NewBlock` message, a call to retrieve the block
/// will succeed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordEvent {
    /// A new block has been added, walk forward (or back, if back, then a state invalidated event
    /// will also be pushed out if relevant)
    NewBlock { uncled: bool, hash: U256 },
    /// A new transaction that has come into the system and is now pending
    NewTxn { hash: U256 },
    /// The state needs to be transitioned backwards, probably onto a new branch
    StateInvalidated { new_height: u64, after_height: u64, after_tick: u64 },
}
impl Event for RecordEvent {}


/// An abstraction on the concept of states and state state data. Builds higher-level functionality
/// On top of the database. The implementation uses RwLocks to provide many read, single write
/// thread safety.
///
/// TODO: Also add a block to the known blocks if it is only referenced.
/// TODO: Also allow for reaching out to the network to request missing information.
/// TODO: Allow removing state data for shards which are not being processed.
pub struct RecordKeeper {
    db: RwLock<Database>,
    rules: RwLock<MutationRules>,
    pending_txns: RwLock<HashMap<U256, Txn>>,

    record_listeners: RwLock<Vec<Weak<EventListener<RecordEvent>>>>,
    game_listeners: RwLock<Vec<Weak<EventListener<PlotEvent>>>>,
}

impl RecordKeeper {
    /// Construct a new RecordKeeper from an already opened database and possibly an existing set of
    /// rules.
    pub fn new(db: Database, rules: Option<MutationRules>) -> RecordKeeper {
        RecordKeeper {
            db: RwLock::new(db),
            rules: RwLock::new(rules.unwrap_or(MutationRules::new())),
            pending_txns: RwLock::new(HashMap::new()),
            record_listeners: RwLock::new(Vec::new()),
            game_listeners: RwLock::new(Vec::new())
        }
    }

    /// Construct a new RecordKeeper by opening a database. This will create a new database if the
    /// one specified does not exist. By default, it will open the database in the directory
    /// `env::get_storage_dir()`. See also `Database::open::`.
    /// # Warning
    /// Any database which is opened, is assumed to contain data in a certain way, any outside
    /// modifications can cause undefined behavior.
    pub fn open(path: Option<PathBuf>, rules: Option<MutationRules>) -> Result<RecordKeeper, Error> {
        let db = Database::open(path)?;
        Ok(Self::new(db, rules))
    }


    pub fn create_block(&self, txns: HashSet<U256>) -> Block {
        unimplemented!("Need to create blocks from transactions")
    }

    /// Add a new block to the chain state after verifying it is valid. Then check pending
    /// transaction to see which ones are no longer pending, and to see which ones have been
    /// invalidated. Also move the network state to be at the new end of the chain.
    pub fn add_block(&mut self, block: &Block) -> Result<(), Error> {
        let block_hash = block.header.calculate_hash();
        let block_height = self.get_block_height(&block.header.prev)?;
        self.add_block_to_height(block_height, block_hash)?;

        unimplemented!("Need to add a block and move the state forward if it is longer than the current chain")
    }

    /// Add a new transaction to the pool of pending transactions.
    pub fn add_pending_txn(&mut self, txn: &Txn) -> Result<(), Error> {
        unimplemented!()
    }

    /// Find a validator's public key given the hash. If they are not found, then they are not a
    /// validator.
    pub fn get_validator_key(&self, id: &U160) -> Result<Vec<u8>, Error> {
        unimplemented!()
    }

    /// Get the reputation of a validator. Will default to 0 if they are not found.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_rep(&self, id: &U160) -> Result<i64, Error> {
        unimplemented!()
    }

    /// Retrieve the current block hash which the network state represents.
    pub fn get_current_block_hash(&self) -> Result<U256, Error> {
        let db_lock = self.db.read().unwrap();
        Self::get_current_block_hash_given_lock(&*db_lock)
    }

    /// Retrieve the header of the current block which the network state represents.
    pub fn get_current_block_header(&self) -> Result<BlockHeader, Error> {
        let db_lock = self.db.read().unwrap();
        let hash = Self::get_current_block_hash_given_lock(&*db_lock)?;
        Self::get_block_header_given_lock(&*db_lock, &hash)
    }

    /// Retrieve the current block which the network state represents.
    pub fn get_current_block(&self) -> Result<Block, Error> {
        let db_lock = self.db.read().unwrap();
        let hash = Self::get_current_block_hash_given_lock(&*db_lock)?;
        Self::get_block_given_lock(&*db_lock, &hash)
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

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    pub fn get_blocks_of_height(&self, height: u64) -> Result<HashSet<U256>, Error> {
        let key = Self::get_block_height_key(height);
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

    /// Returns a list of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `after_tick` simply allows additional filtering, e.g. if
    /// you set `after_tick` to 0, you would not get all events unless those events have not yet
    /// been removed from the cache.
    pub fn get_plot_events(&self, plot_id: u64, after_tick: u64) -> Events<PlotEvent> {
        unimplemented!()
    }

    /// Add a new listener for events such as new blocks. This will also take a moment to remove any
    /// listeners which no longer exist.
    pub fn register_record_listener(&mut self, listener: &Arc<EventListener<RecordEvent>>) {
        let mut listeners = self.record_listeners.write().unwrap();
        listeners.retain(|l| l.upgrade().is_some());
        listeners.push(Arc::downgrade(listener));
    }

    /// Add a new listener for plot events. This will also take a moment to remove any listeners
    /// which no longer exist.
    pub fn register_game_listener(&mut self, listener: &Arc<EventListener<PlotEvent>>) {
        let mut listeners = self.game_listeners.write().unwrap();
        listeners.retain(|l| l.upgrade().is_some());
        listeners.push(Arc::downgrade(listener));
    }

    /// Add a new rule to the database regarding what network mutations are valid. This will only
    /// impact things which are mutated through the `mutate` function.
    pub fn add_rule(&mut self, rule: Box<MutationRule>) {
        let mut rules_lock = self.rules.write().unwrap();
        rules_lock.push_back(rule);
    }

    /// Check if a mutation to the network state is valid.
    pub fn is_valid(&self, mutation: &Mutation) -> Result<(), String> {
        let db_lock = self.db.read().unwrap();
        self.is_valid_given_lock(&*db_lock, mutation)
    }
    
    /// Retrieve raw data from the database. Use this for non-storable types (mostly network stuff).
    pub fn get_raw_data(&self, key: &[u8], postfix: &'static [u8]) -> Result<Vec<u8>, Error> {
        let db_lock = self.db.read().unwrap();
        db_lock.get_raw_data(key, postfix)
    }

    /// Put raw data into the database. Should have no uses outside this class.
    fn put_raw_data(&mut self, key: &[u8], data: &[u8], postfix: &'static [u8]) -> Result<(), Error> {
        let mut db_lock = self.db.write().unwrap();
        db_lock.put_raw_data(key, data, postfix)
    }

    /// Retrieve cache data from the database. This is for library use only.
    pub fn get_cache_data<S: Storable>(&self, instance_id: &[u8]) -> Result<S, Error> {
        let db_lock = self.db.read().unwrap();
        Self::get::<S>(&*db_lock, instance_id, CACHE_POSTFIX)
    } //TODO: handle possible race condition?

    /// Put cache data into the database. This is for library use only.
    pub fn put_cache_data<S: Storable>(&mut self, obj: &S) -> Result<(), Error> {
        let mut db_lock = self.db.write().unwrap();
        Self::put::<S>(&mut *db_lock, obj, CACHE_POSTFIX)
    }

    /// Retrieve a block header from the database.
    pub fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        let db_lock = self.db.read().unwrap();
        Self::get_block_header_given_lock(&*db_lock, hash)
    }

    /// Get a block including its list of transactions from the database.
    pub fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        let db_lock = self.db.read().unwrap();
        Self::get_block_given_lock(&*db_lock, hash)
    }

    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let db_lock = self.db.read().unwrap();
        Self::complete_block_given_lock(&*db_lock, header)
    }

    /// Get a transaction from the database.
    pub fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let db_lock = self.db.read().unwrap();
        Self::get_txn_given_lock(&*db_lock, hash)
    }


    /// Add a block the set of known blocks at a given height.
    fn add_block_to_height(&mut self, height: u64, block: U256) -> Result<(), Error> {
        let key = Self::get_block_height_key(height);
        let mut db = self.db.write().unwrap(); // single lock through whole process
        let res = db.get_raw_data(&key, CACHE_POSTFIX);
        
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
        db.put_raw_data(&key, &raw, CACHE_POSTFIX)
    }

    /// Check if a mutation is valid and then apply the changes to the network state.
    fn mutate(&mut self, mutation: &Mutation) -> Result<Mutation, Error> {
        // mutation.assert_not_contra();
        // let mut db_lock = self.db.write().unwrap();
        // self.is_valid_given_lock(&*db_lock, mutation).map_err(|e| Error::InvalidMut(e))?;
        
        // db_lock.mutate(mutation)

        unimplemented!()
    }

    /// Apply a contra mutation to the network state. (And consumes the mutation).
    fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), Error> {
        // mutation.assert_contra();
        // let mut db_lock = self.db.write().unwrap();
        
        // db_lock.undo_mutate(mutation)

        unimplemented!()
    }

    /// Internal use function to check if a mutation is valid given a lock of the db. While it only
    /// takes a reference to a db, make sure a lock is in scope which is used to get the db ref.
    fn is_valid_given_lock(&self, db: &Database, mutation: &Mutation) -> Result<(), String> {
        let rules_lock = self.rules.read().unwrap();
        for rule in &*rules_lock {
            // verify all rules are satisfied and return, propagate error if not
            rule.is_valid(db, mutation)?;
        }use std::collections::BTreeMap;
        Ok(())
    }


    /// Retrieve and deserialize data from the database. This will return an error if the database
    /// has an issue, if the data cannot be deserialized or if the object is not present in the
    /// database. Note that `instance_id` should be the object's ID/key which would normally be
    /// returned from calling `storable.instance_id()`.
    fn get<S: Storable>(db: &Database, instance_id: &[u8], postfix: &'static [u8]) -> Result<S, Error> {
        let key = {
            let mut k = Vec::from(S::global_id());
            k.extend_from_slice(instance_id); k
        };

        let raw = db.get_raw_data(&key, postfix)?;
        Ok(bincode::deserialize::<S>(&raw)?)
    }

    /// Serialize and store data in the database. This will return an error if the database has an
    /// issue.
    fn put<S: Storable>(db: &mut Database, obj: &S, postfix: &'static [u8]) -> Result<(), Error> {
        let value = bincode::serialize(obj, bincode::Infinite)
            .expect("Error serializing game data.");
        db.put_raw_data(&obj.key(), &value, postfix)
    }

    /// Get the key value for the height cache in the database. (Without the cache postfix).
    fn get_block_height_key(height: u64) -> Vec<u8> {
        let mut buf = [0u8; 8];
        BigEndian::write_u64(&mut buf, height);
        let mut k = Vec::from(HEIGHT_PREFIX);
        k.extend_from_slice(&buf); k
    }

    fn get_current_block_hash_given_lock(db: &Database) -> Result<U256, Error> {
        unimplemented!()
    }

    fn get_block_header_given_lock(db: &Database, hash: &U256) -> Result<BlockHeader, Error> {
        let id = hash.to_vec();
        Self::get::<BlockHeader>(db, &id, BLOCKCHAIN_POSTFIX)
    }

    fn get_block_given_lock(db: &Database, hash: &U256) -> Result<Block, Error> {
        // Blocks are stored with their header separate from the transaction body, so get the header
        // first to find the merkle_root, and then get the list of transactions and piece them
        // together.
        let header = Self::get_block_header_given_lock(db, hash)?;
        Self::complete_block_given_lock(db, header)
    }

    fn complete_block_given_lock(db: &Database, header: BlockHeader) -> Result<Block, Error> {
        let merkle_root = header.merkle_root.to_vec();
        let raw_txns = db.get_raw_data(&merkle_root, BLOCKCHAIN_POSTFIX)?;
        Ok(Block::deserialize(header, &raw_txns)?)
    }

    fn get_txn_given_lock(db: &Database, hash: &U256) -> Result<Txn, Error> {
        let id = hash.to_vec();
        Self::get::<Txn>(db, &id, BLOCKCHAIN_POSTFIX)
    }
}