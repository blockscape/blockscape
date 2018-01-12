use bin::Bin;
use openssl::pkey::PKey;
use primitives::{U256, U160, Txn, Block, BlockHeader, Mutation, Change, ListenerPool};
use std::collections::{HashMap, BTreeSet, BTreeMap};
use std::path::PathBuf;
use std::sync::RwLock;
use super::{MutationRule, MutationRules, Error, LogicError, Storable, RecordEvent, PlotID};
use super::{PlotEvent, PlotEvents, events};
use super::BlockPackage;
use super::database::*;
use time::Time;
use hash::hash_pub_key;

use futures::sync::mpsc::Sender;
use futures_cpupool;

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

    /// The hash of this users public key
    key_hash: U160,
    /// This user's private key
    key: PKey,

    /// A CPU pool of a single thread dedicated to processing work related to RecordKeeper. Work can be passed to it. Future compatible.
    /// It is reccomended to spawn major work for the DB on this thread; one can also spawn their own thread for smaller, higher priority jobs.
    worker: futures_cpupool::CpuPool
}

impl RecordKeeper {
    /// Construct a new RecordKeeper from an already opened database and possibly an existing set of
    /// rules.
    pub fn new(db: Database, rules: Option<MutationRules>, key: PKey) -> RecordKeeper {
        RecordKeeper {
            db: RwLock::new(db),
            rules: RwLock::new(rules.unwrap_or(MutationRules::new())),
            pending_txns: RwLock::new(HashMap::new()),
            record_listeners: RwLock::new(ListenerPool::new()),
            game_listeners: RwLock::new(ListenerPool::new()),
            key_hash: hash_pub_key(&key.public_key_to_der().unwrap()),
            key,
            worker: futures_cpupool::Builder::new().pool_size(1).create()
        }
    }

    pub fn get_worker(&self) -> &futures_cpupool::CpuPool {
        &self.worker
    }

    /// Construct a new RecordKeeper by opening a database. This will create a new database if the
    /// one specified does not exist. By default, it will open the database in the directory
    /// `env::get_storage_dir()`. See also `Database::open::`.
    /// # Warning
    /// Any database which is opened, is assumed to contain data in a certain way, any outside
    /// modifications can cause undefined behavior.
    pub fn open(key: PKey, path: Option<PathBuf>, rules: Option<MutationRules>) -> Result<RecordKeeper, Error> {
        let db = Database::open(path)?;
        Ok(Self::new(db, rules, key))
    }

    /// Use pending transactions to create a new block which can then be added to the network.
    pub fn create_block(&self) -> Result<Block, Error> {
        let pending_txns = self.pending_txns.read().unwrap();
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
                    merkle_root: Block::calculate_merkle_root(&BTreeSet::new()),
                    blob: Bin::new(),
                    creator: self.key_hash,
                    signature: Bin::new()
                }.sign(&self.key),
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

        for (_time, txn) in txns_by_time {
            last_block = Some(Block{
                header: BlockHeader{
                    version: 1,
                    timestamp: Time::current(),
                    shard: if cbh.shard.is_zero() { cbh_h } else { cbh.shard },
                    prev: cbh_h,
                    merkle_root: Block::calculate_merkle_root(&accepted_txns),
                    blob: Bin::new(),
                    creator: self.key_hash,
                    signature: Bin::new()
                }.sign(&self.key),
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

    /// Add a new block and its associated transactions to the chain state after verifying
    /// it is valid. Also move the network state to be at the new end of the chain.
    /// Returns true if the block was added, false if it was already in the system.
    pub fn add_block(&self, block: &Block) -> Result<bool, Error> {
        let mut pending_txns = self.pending_txns.write().unwrap();
        let mut db = self.db.write().unwrap();

        self.is_valid_block_given_lock(&*db, block)?;

        // we know it is a valid block, so go ahead and add it's transactions, and then it.
        for txn_hash in block.txns.iter() {
            if let Some(txn) = pending_txns.remove(&txn_hash) { // we will need to add it 
                db.add_txn(&txn)?;
            } else {
                // should already be in the DB then because otherwise is_valid_block should give an
                // error, so use an assert check
                assert!(db.get_txn(&txn_hash).is_ok())
            }
        }
        // add txns first, so that we make sure all blocks in the system have all their information,
        // plus, any txn which is somehow added without its block will probably be added by another
        // anyway so it is the lesser of the evils

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
    /// the list of pending transactions or accepted into the database..
    pub fn add_pending_txn(&self, txn: &Txn) -> Result<bool, Error> {
        let hash = txn.calculate_hash();

        let mut txns = self.pending_txns.write().unwrap();
        let db = self.db.read().unwrap();
        
        // check if it is already pending
        if txns.contains_key(&hash) {
            return Ok(false);
        }

        // check if it is already in the database
        match db.get_txn(&hash) {
            Ok(_) => return Ok(false),
            Err(Error::NotFound(..)) => {},
            Err(e) => return Err(e)
        }

        self.is_valid_txn_given_lock(&db, txn)?;
        txns.insert(hash, txn.clone());
        Ok(true)
    }

    /// Import a package of blocks and transactions.
    pub fn import_pkg(&self, pkg: BlockPackage) -> Result<(), Error> {
        let (blocks, txns) = pkg.unpack();
        for txn in txns {
            self.add_pending_txn(&txn.1)?;
        } for block in blocks {
            self.add_block(&block)?;
        } Ok(())
    }

    /// Find a validator's public key given the hash. If they are not found, then they are not a
    /// validator.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_key(&self, id: &U160) -> Result<Bin, Error> {
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
    pub fn get_blocks_of_height(&self, height: u64) -> Result<Vec<U256>, Error> {
        let db = self.db.read().unwrap();
        db.get_blocks_of_height(height)
    }

    /// Get a list of the last `count` block headers. If `count` is one, then it will return only
    /// the most recent block.
    pub fn get_latest_blocks(&self, count: usize) -> Result<Vec<BlockHeader>, Error> {
        let db = self.db.read().unwrap();
        db.get_latest_blocks(count)
    }
    
    /// Get blocks before the `target` hash until it collides with the main chain. If the `start`
    /// hash lies between the target and the main chain, it will return the blocks between them,
    /// otherwise it will return the blocks from the main chain until target in that order and it
    /// will not include the start or target blocks.
    ///
    /// If the limit is reached, it will prioritize blocks of a lower height, but may have a gap
    /// between the main chain (or start) and what it includes.
    pub fn get_blocks_before(&self, last_known: &U256, target: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let db = self.db.read().unwrap();
        BlockPackage::blocks_before(&*db, last_known, target, limit)
    }

    /// Create a `BlockPackage` of all the blocks of the current chain which are a descendent of the
    /// latest common ancestor between the chain of the start block and the current chain. It will
    /// not include the start block. The `limit` is the maximum number of bytes the final package
    /// may contain.
    pub fn get_blocks_after(&self, start: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let db = self.db.read().unwrap();
        BlockPackage::blocks_after(&*db, start, limit)
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
    pub fn register_record_listener(&self, listener: Sender<RecordEvent>) {
        self.record_listeners.write().unwrap().register(listener);
    }

    /// Add a new listener for plot events. This will also take a moment to remove any listeners
    /// which no longer exist.
    pub fn register_game_listener(&self, listener: Sender<PlotEvent>) {
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
        match pending.get(&hash) {
            Some(txn) => Ok(txn.clone()),
            None => db.get_txn(hash)
        }
    }


    /// Internal use function to check if a block and all its sub-components are valid.
    fn is_valid_block_given_lock(&self, db: &Database, block: &Block) -> Result<(), Error> {
        // if match db.get_validator_key(&block.creator) {
        //     Ok(key) => {
        //         let key = PKey::public_key_from_der(&key).unwrap();
        //         block.verify_signature(&key)
        //     }
        //     Err(Error::NotFound(..)) => return Err(LogicError::UnrecognizedCreator.into()),
        //     Err(e) => return Err(e)
        // } {/* signature is value */}
        // else { return Err(LogicError::InvalidSignature.into())}
        
        // match db.get_block_header(&block.prev) {
        //     Ok(_) => {},
        //     Err(Error::NotFound(..)) => return Err(LogicError::MissingPrevious.into()),
        //     Err(e) => return Err(e)
        // }

        // //TODO: more validity checks

        // let mut mutation = Mutation::new();
        // for txn_hash in &block.txns {
        //     let txn = self.get_txn_given_lock(db, &txn_hash)?;
        //     Self::is_valid_txn_header_given_lock(db, &txn)?;
        //     mutation.merge_clone(&txn.mutation);
        // }
        
        // self.is_valid_mutation_given_lock(db, &mutation)
        unimplemented!()
    }

    /// Internal use function, check if a txn is valid and its mutation.
    fn is_valid_txn_given_lock(&self, db: &Database, txn: &Txn) -> Result<(), Error> {
        // Self::is_valid_txn_header_given_lock(db, txn)?;
        // self.is_valid_mutation_given_lock(db, &txn.mutation)
        unimplemented!()
    }

    /// Internal use function, check if the basics of a txn is valid, ignore its mutations.
    fn is_valid_txn_header_given_lock(db: &Database, txn: &Txn) -> Result<(), Error> {
        // //TODO: validity checks on things like timestamp, signature, and that the public key is of someone we know
        
        // // verify the txn is not already part of the blockchain
        // if let Ok(_) = db.get_raw_data(&txn.calculate_hash().to_vec(), BLOCKCHAIN_POSTFIX) {
        //     return Err(Error::from(LogicError::Duplicate))
        // }

        // // It is valid on the surface
        // Ok(())
        unimplemented!()
    }

    /// Internal use function to check if a mutation is valid given a lock of the db.
    fn is_valid_mutation_given_lock(&self, db: &Database, mutation: &Mutation) -> Result<(), Error> {
        // let rules = self.rules.read().unwrap();
        // let mut cache = Bin::new();
        // for rule in &*rules {
        //     // verify all rules are satisfied and return, propagate error if not
        //     rule.is_valid(db, mutation, &mut cache)?;
        // }
        // Ok(())
        unimplemented!()
    }

    // fn get_txn_given_lock(&self, db: &Database, hash: &U256) -> Result<Txn, Error> {
    //     if let Some(txn) = self.pending_txns.read().unwrap().get(hash) {
    //         Ok(txn.clone())
    //     } else {
    //         db.get_txn(hash)
    //     }
    // }
}