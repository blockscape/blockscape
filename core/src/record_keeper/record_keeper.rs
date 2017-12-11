use primitives::{U256, U160, Txn, Block, BlockHeader, Mutation, Change, EventListener, ListenerPool};
use std::collections::{HashMap, HashSet, BTreeSet, BTreeMap};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use super::{MutationRule, MutationRules, Error, LogicError, Storable, RecordEvent, PlotID};
use super::{PlotEvent, PlotEvents, events};
use super::database::*;
use time::Time;

/// An abstraction on the concept of states and state state data. Builds higher-lsuperevel functionality
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
    pub fn create_block(&self) -> Result<Block, Error> {
        let pending_txns = self.pending_txns.read().unwrap();
        let rules = self.rules.read().unwrap();
        let db = self.db.read().unwrap();

        let cbh = db.get_current_block_header()?;
        let cbh_h = cbh.calculate_hash();

        if pending_txns.is_empty() {
            return Ok(Block{
                header: BlockHeader{
                    version: 1,
                    timestamp: Time::current(),
                    shard: if cbh.shard.is_zero() { cbh_h } else { cbh.shard },
                    prev: cbh_h,
                    merkle_root: Block::calculate_merkle_root(&BTreeSet::new())
                },
                txns: BTreeSet::new()
            })
        }

        // sort txns by their timestamp
        let txns_by_time: BTreeMap<Time, &U256> = 
            pending_txns.iter()
                .map(|(txn_h, txn)| (txn.timestamp, txn_h))
                .collect();

        let mut accepted_txns: BTreeSet<U256> = BTreeSet::new();
        let mut last_block = None;

        for (time, txn) in txns_by_time {
            last_block = Some(Block{
                header: BlockHeader{
                    version: 1,
                    timestamp: Time::current(),
                    shard: if cbh.shard.is_zero() { cbh_h } else { cbh.shard },
                    prev: cbh_h,
                    merkle_root: Block::calculate_merkle_root(&accepted_txns)
                },
                txns: {
                    let mut t = accepted_txns.clone();
                    t.insert(*txn); t
                }
            });

            let res = self.is_valid_block(last_block.as_ref().unwrap());
            use self::Error::*;
            if let Ok(()) = res {
                accepted_txns.insert(*txn);
            } else {
                match res.err().unwrap() {
                    Logic(..) => {}, // do nothing
                    e @ _ => return Err(e) // pass along the error
                }
            }
        }

        Ok(last_block.unwrap())
    }

    /// Add a new block to the chain state after verifying it is valid. Then check pending
    /// transaction to see which ones are no longer pending, and to see which ones have been
    /// invalidated. Also move the network state to be at the new end of the chain.
    /// Returns true if the block was added, false if it was already in the system.
    pub fn add_block(&self, block: &Block) -> Result<bool, Error> {
        let block_hash = block.header.calculate_hash();
        let mut db = self.db.write().unwrap();

        self.is_valid_block_given_lock(&*db, block)?;

        // record the block
        if db.add_block(block)? {
            db.walk_to_head()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Add a new transaction to the pool of pending transactions after validating it. Returns true
    /// if it was added successfully to pending transactions, and returns false if it is already in
    /// the list of pending transactions.
    pub fn add_pending_txn(&self, txn: &Txn) -> Result<bool, Error> {
        let hash = txn.calculate_hash();

        let mut txns = self.pending_txns.write().unwrap();
        let db = self.db.read().unwrap();
        
        if txns.contains_key(&hash) {
            return Ok(false);
        }

        self.is_valid_txn_given_lock(&db, txn)?;
        txns.insert(hash, txn.clone());
        Ok(true)
    }

    /// Find a validator's public key given the hash. If they are not found, then they are not a
    /// validator.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_key(&self, id: &U160) -> Result<Vec<u8>, Error> {
        self.db.read().unwrap()
            .get_validator_key(id)
    }

    /// Get the reputation of a validator given their ID.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_rep(&self, id: &U160) -> Result<i64, Error> {
        self.db.read().unwrap()
            .get_validator_rep(id)
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

    /// Function to find the unknown blocks from the last known block until the desired block. It
    /// will never include the `last_known` or `target` blocks in the result and the result will be
    /// in order from lowest height the greatest height.
    ///
    /// In summary, it will always find the latest common ancestor of the two blocks and then
    /// traverse upwards until it reaches the target and only return those found when traversing
    /// upwards.
    ///
    /// Main -> Main will retrieve all blocks which are descendants of start and ancestors of target
    /// and will not include start or target.
    ///
    /// Main -> Uncle will yield all blocks after the start block until the uncle going directly up
    /// the chain and then over. I.e. it will go up the chain and fork off to the branch the uncle
    /// is on and go up that.
    ///
    /// Uncle -> Main will yield all descendants of the latest common ancestor of start with the
    /// main chain until the target block. I.e. it will back up to the main chain and then go until
    /// it reaches the new block.
    ///
    /// Uncle -> Uncle will retrieve all blocks along the path between the uncles. This may traverse
    /// down to the main chain and then back up to the uncle if they are on different offshoots.
    pub fn get_unknown_blocks(&self, last_known: &U256, target: &U256, limit: u32) -> Result<Vec<U256>, Error> {
        self.db.read().unwrap()
            .get_unknown_blocks(last_known, target, limit)
    }

    /// Retrieves all the blocks of the current chain which are a descendent of the latest common
    /// ancestor between the chain of the start block and the current chain. This result will be
    /// sorted in ascending height order. It will not include the start hash.
    pub fn get_blocks_after_hash(&self, start: &U256, limit: u32) -> Result<Vec<U256>, Error> {
        self.db.read().unwrap()
            .get_blocks_after_hash(start, limit)
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
        let db = self.db.read().unwrap();
        self.is_valid_block_given_lock(&*db, block)
    }

    /// Check if a txn is valid given the current network state including all of its mutations.
    pub fn is_valid_txn(&self, txn: &Txn) -> Result<(), Error> {
        let db = self.db.read().unwrap();
        self.is_valid_txn_given_lock(&*db, txn)
    }

    /// Check if a mutation is valid given the current network state.
    pub fn is_valid_mutation(&self, mutation: &Mutation) -> Result<(), Error> {
        let db = self.db.read().unwrap();
        self.is_valid_mutation_given_lock(&*db, mutation)
    }

    /// Retrieve cache data from the database. This is for library use only.rules: &MutationRules,
    pub fn get_cache_data<S: Storable>(&self, instance_id: &[u8]) -> Result<S, Error> {
        let db = self.db.read().unwrap();
        db.get::<S>(instance_id, CACHE_POSTFIX)
    }

    /// Put cache data into the database. This is for library use only.
    pub fn put_cache_data<S: Storable>(&self, obj: &S) -> Result<(), Error> {
        let mut db = self.db.write().unwrap();
        db.put::<S>(obj, CACHE_POSTFIX)
    }

    /// Retrieve a block header from the database.
    pub fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        let db = self.db.read().unwrap();
        db.get_block_header(hash)
    }

    /// Get a block including its list of transactions from the database.
    pub fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        let db = self.db.read().unwrap();
        db.get_block(hash)
    }

    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let db = self.db.read().unwrap();
        db.complete_block(header)
    }

    /// Get a transaction from the database.
    pub fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let pending = self.pending_txns.read().unwrap();
        let db = self.db.read().unwrap();
        self.get_txn_given_lock(&*db, hash)
    }


    /// Internal use function to check if a block and all its sub-components are valid.
    fn is_valid_block_given_lock(&self, db: &Database, block: &Block) -> Result<(), Error> {
        if block.prev != db.get_current_block_hash() {
            return Err(Error::from(LogicError::MissingPrevious));
        }

        //TODO: more validity checks

        let mut mutation = Mutation::new();
        for txn_hash in &block.txns {
            let txn = self.get_txn_given_lock(db, &txn_hash)?;
            Self::is_valid_txn_header_given_lock(db, &txn)?;
            mutation.merge_clone(&txn.mutation);
        }
        
        self.is_valid_mutation_given_lock(db, &mutation)
    }

    /// Internal use function, check if a txn is valid and its mutation.
    fn is_valid_txn_given_lock(&self, db: &Database, txn: &Txn) -> Result<(), Error> {
        Self::is_valid_txn_header_given_lock(db, txn)?;
        self.is_valid_mutation_given_lock(db, &txn.mutation)
    }

    /// Internal use function, check if the basics of a txn is valid, ignore its mutations.
    fn is_valid_txn_header_given_lock(db: &Database, txn: &Txn) -> Result<(), Error> {
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
        let rules = self.rules.read().unwrap();
        for rule in &*rules {
            // verify all rules are satisfied and return, propagate error if not
            rule.is_valid(db, mutation).map_err(|e| LogicError::InvalidMutation(e))?;
        }
        Ok(())
    }

    fn get_txn_given_lock(&self, db: &Database, hash: &U256) -> Result<Txn, Error> {
        if let Some(txn) = self.pending_txns.read().unwrap().get(hash) {
            Ok(txn.clone())
        } else {
            db.get_txn(hash)
        }
    }
}