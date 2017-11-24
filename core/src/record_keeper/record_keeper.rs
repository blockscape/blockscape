use primitives::{U256, U160, Txn, Block, BlockHeader, Mutation, Change, EventListener, ListenerPool};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use super::{MutationRule, MutationRules, Error, LogicError, Storable, RecordEvent, PlotID};
use super::{PlotEvent, PlotEvents, events};
use super::database::*;

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

    record_listeners: RwLock<ListenerPool<RecordEvent>>,
    game_listeners: RwLock<ListenerPool<PlotEvent>>,
}

impl RecordKeeper {
    /// Construct a new RecordKeeper from an already opened database and possibly an existing set of
    /// rules.
    pub fn new(db: Database, rules: Option<MutationRules>) -> RecordKeeper {
        RecordKeeper {
            db: RwLock::new(db),
            rules: RwLock::new(rules.unwrap_or(MutationRules::new())),
            pending_txns: RwLock::new(HashMap::new()),
            record_listeners: RwLock::new(ListenerPool::new()),
            game_listeners: RwLock::new(ListenerPool::new())
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

    /// Use pending transactions to create a new block which can then be added to the network.
    pub fn create_block(&self, txns: HashSet<U256>) -> Block {
        unimplemented!("Need to create blocks from transactions")
    }

    /// Add a new block to the chain state after verifying it is valid. Then check pending
    /// transaction to see which ones are no longer pending, and to see which ones have been
    /// invalidated. Also move the network state to be at the new end of the chain.
    /// Returns true if the block was added, false if it was already in the system.
    pub fn add_block(&self, block: &Block) -> Result<bool, Error> {
        let block_hash = block.header.calculate_hash();
        let block_height = self.get_block_height(&block.header.prev)?;
        
        let mut db = self.db.write().unwrap();

        // Check if the block is already in the system
        if db.get_raw_data(&block_hash.to_vec(), BLOCKCHAIN_POSTFIX).is_ok() {
            return Ok(false);
        }

        //TODO: Handle if we need to go back and switch branches
        //TODO: Handle if we are only recording the block, and it does not become part of the current chain
        self.is_valid_block_given_lock(&*db, block)?;

        db.add_block_to_height(block_height, block_hash)?;
        
        let mut mutation = Mutation::new();
        for txn_hash in &block.transactions {
            let txn = db.get_txn(&txn_hash)?;
            mutation.merge_clone(&txn.mutation);
        }
        let contra = db.mutate(&mutation)?;
        db.add_contra(&block_hash, &contra)?;
        db.update_current_block(&block_hash, Some(block_height))?;
        Ok(true)
    }

    /// Step the network state back one block to the previous in the chain.
    /// This will throw an error if it is asked to step back over an origin block.
    pub fn step_back(&self) -> Result<(), Error> {
        let mut db = self.db.write().unwrap();
        let start_hash = db.get_current_block_hash();
        let start_block = db.get_block_header(&start_hash)?;
        let head = db.get_block_header(&start_hash)?;
        if head.shard.is_zero() { return Err(Error::from(LogicError::UndoOrigin)) }
        let contra = db.get_contra(&start_hash)?;
        db.undo_mutate(contra)?;
        db.update_current_block(&start_block.prev, None)
    }

    /// Add a new transaction to the pool of pending transactions after validating it. Returns true
    /// if it was added successfully to pending transactions, and returns false if it is already in
    /// the list of pending transactions.
    pub fn add_pending_txn(&self, txn: Txn) -> Result<bool, Error> {
        let hash = txn.calculate_hash();

        let mut txns = self.pending_txns.write().unwrap();
        let db = self.db.read().unwrap();
        
        if txns.contains_key(&hash) {
            return Ok(false);
        }

        self.is_valid_txn_given_lock(&db, &txn)?;
        txns.insert(hash, txn);
        Ok(true)
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
    pub fn get_current_block_hash(&self) -> U256 {
        self.db.read().unwrap().get_current_block_hash()
    }

    /// Retrieve the header of the current block which the network state represents.
    pub fn get_current_block_header(&self) -> Result<BlockHeader, Error> {
        let db = self.db.read().unwrap();
        let hash = db.get_current_block_hash();
        db.get_block_header(&hash)
    }

    /// Retrieve the current block which the network state represents.
    pub fn get_current_block(&self) -> Result<Block, Error> {
        let db = self.db.read().unwrap();
        let hash = db.get_current_block_hash();
        db.get_block(&hash)
    }

    /// Calculate the height of a given block. It will follow the path until it finds the genesis
    /// block which is denoted by having a previous block reference of 0.
    pub fn get_block_height(&self, hash: &U256) -> Result<u64, Error> {
        let db = self.db.read().unwrap();
        db.get_block_height(hash)
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    pub fn get_blocks_of_height(&self, height: u64) -> Result<HashSet<U256>, Error> {
        let db = self.db.read().unwrap();
        db.get_blocks_of_height(height)
    }

    pub fn get_blocks_between(start: U256, target: U256, limit: u32) -> Vec<U256> {
        unimplemented!()
    }

    pub fn get_blocks_after_hash(start: U256, limit: u32) -> Vec<U256> {
        unimplemented!()
    }

    pub fn get_blocks_after_height(start: u64, limit: u32) -> Vec<U256> {
        unimplemented!()
    }

    /// Returns a map of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `after_tick` simply allows additional filtering, e.g. if
    /// you set `after_tick` to 0, you would not get all events unless the oldest events have not
    /// yet been removed from the cache.
    pub fn get_plot_events(&self, plot_id: PlotID, after_tick: u64) -> Result<PlotEvents, Error> {
        let mut events: PlotEvents = {
            let db = self.db.read().unwrap();
            db.get_plot_events(plot_id, after_tick)?
        };
        
        let txns = self.pending_txns.read().unwrap();
        for txn in txns.values() {
            for change in &txn.mutation.changes {
                if let &Change::AddEvent{id, tick, ref event, ..} = change {
                    if tick >= after_tick && id == plot_id {
                        events::add_event(&mut events, tick, event.clone());
                    }
                }
            }
        }
        
        Ok(events)
    }

    /// Add a new listener for events such as new blocks. This will also take a moment to remove any
    /// listeners which no longer exist.
    pub fn register_record_listener(&self, listener: &Arc<EventListener<RecordEvent>>) {
        self.record_listeners.write().unwrap().register(listener);
    }

    /// Add a new listener for plot events. This will also take a moment to remove any listeners
    /// which no longer exist.
    pub fn register_game_listener(&self, listener: &Arc<EventListener<PlotEvent>>) {
        self.game_listeners.write().unwrap().register(listener);
    }

    /// Add a new rule to the database regarding what network mutations are valid. This will only
    /// impact things which are mutated through the `mutate` function.
    pub fn add_rule(&mut self, rule: Box<MutationRule>) {
        let mut rules_lock = self.rules.write().unwrap();
        rules_lock.push_back(rule);
    }

    /// Check if a block is valid given the current network state including all of its transactions
    /// and the resulting mutations.
    pub fn is_valid_block(&self, block: &Block) -> Result<(), Error> {
        let db_lock = self.db.read().unwrap();
        self.is_valid_block_given_lock(&*db_lock, block)
    }

    /// Check if a txn is valid given the current network state including all of its mutations.
    pub fn is_valid_txn(&self, txn: &Txn) -> Result<(), Error> {
        let db_lock = self.db.read().unwrap();
        self.is_valid_txn_given_lock(&*db_lock, txn)
    }

    /// Check if a mutation is valid given the current network state.
    pub fn is_valid_mutation(&self, mutation: &Mutation) -> Result<(), Error> {
        let db_lock = self.db.read().unwrap();
        self.is_valid_mutation_given_lock(&*db_lock, mutation)
    }

    /// Retrieve cache data from the database. This is for library use only.
    pub fn get_cache_data<S: Storable>(&self, instance_id: &[u8]) -> Result<S, Error> {
        let db_lock = self.db.read().unwrap();
        db_lock.get::<S>(instance_id, CACHE_POSTFIX)
    }

    /// Put cache data into the database. This is for library use only.
    pub fn put_cache_data<S: Storable>(&self, obj: &S) -> Result<(), Error> {
        let mut db_lock = self.db.write().unwrap();
        db_lock.put::<S>(obj, CACHE_POSTFIX)
    }

    /// Retrieve a block header from the database.
    pub fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        let db_lock = self.db.read().unwrap();
        db_lock.get_block_header(hash)
    }

    /// Get a block including its list of transactions from the database.
    pub fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        let db_lock = self.db.read().unwrap();
        db_lock.get_block(hash)
    }

    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let db_lock = self.db.read().unwrap();
        db_lock.complete_block(header)
    }

    /// Get a transaction from the database.
    pub fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let db_lock = self.db.read().unwrap();
        db_lock.get_txn(hash)
    }

    /// Internal use function to check if a block and all its sub-components are valid.
    fn is_valid_block_given_lock(&self, db: &Database, block: &Block) -> Result<(), Error> {
        if block.prev != self.get_current_block_hash() {
            return Err(Error::from(LogicError::MissingPrevious));
        }

        //TODO: more validity checks

        let mut mutation = Mutation::new();
        for txn_hash in &block.transactions {
            let txn = db.get_txn(&txn_hash)?;
            self.is_valid_txn_header_given_lock(db, &txn)?;
            mutation.merge_clone(&txn.mutation);
        }
        
        self.is_valid_mutation_given_lock(db, &mutation)
    }

    /// Internal use function, check if a txn is valid and its mutation.
    fn is_valid_txn_given_lock(&self, db: &Database, txn: &Txn) -> Result<(), Error> {
        self.is_valid_txn_header_given_lock(db, txn)?;
        self.is_valid_mutation_given_lock(db, &txn.mutation)
    }

    /// Internal use function, check if the basics of a txn is valid, ignore its mutations.
    fn is_valid_txn_header_given_lock(&self, db: &Database, txn: &Txn) -> Result<(), Error> {
        //TODO: validity checks on things like timestamp, signature, and that the public key is of someone we know
        
        // verify the txn is not already part of the blockchain
        if let Ok(_) = db.get_raw_data(&txn.calculate_hash().to_vec(), BLOCKCHAIN_POSTFIX) {
            return Err(Error::from(LogicError::Duplicate))
        }

        // It is valid on the surface
        Ok(())
    }

    /// Internal use function to check if a mutation is valid given a lock of the db.
    fn is_valid_mutation_given_lock(&self, db: &Database, mutation: &Mutation) -> Result<(), Error> {
        let rules_lock = self.rules.read().unwrap();
        for rule in &*rules_lock {
            // verify all rules are satisfied and return, propagate error if not
            rule.is_valid(db, mutation).map_err(|e| LogicError::InvalidMutation(e))?;
        }
        Ok(())
    }
}