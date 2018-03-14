use bin::Bin;
use primitives::{JU256, U256, U160, U160_ZERO, Txn, Block, BlockHeader, Change, ListenerPool};
use std::collections::{HashMap, BTreeSet, HashSet};
use std::path::PathBuf;
use parking_lot::{RwLock, Mutex};
use primitives::{RawEvents, event};
use super::{Error, RecordEvent, PlotEvent, PlotID};
use super::{NetState, NetDiff};
use super::{rules, BlockRule, TxnRule, MutationRule, MutationRules};
use super::BlockPackage;
use super::database::*;
use time::Time;

use futures::sync::mpsc::Sender;
use futures_cpupool;

const MAX_PENDING_TXN_MEM: usize = 128*1024*1024; //128 MB

#[derive(Debug, Serialize)]
/// RK Stats which can be sent via JSON on request.
pub struct RecordKeeperStatistics {
    height: u64,
    current_block_hash: JU256,

    pending_txns_count: u64,
    pending_txns_size: u64,
}


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
    pending_txns: RwLock<HashMap<U256, (Time, Txn)>>,

    record_listeners: Mutex<ListenerPool<RecordEvent>>,
    game_listeners: Mutex<ListenerPool<PlotEvent>>,

    /// A CPU pool of a single thread dedicated to processing work related to RecordKeeper. Work can be passed to it. Future compatible.
    /// It is reccomended to spawn major work for the DB on this thread; one can also spawn their own thread for smaller, higher priority jobs.
    worker: futures_cpupool::CpuPool,

    /// A larger work queue designed for smaller, time sensitive jobs
    priority_worker: futures_cpupool::CpuPool
}

impl RecordKeeper {
    /// Construct a new RecordKeeper from an already opened database and possibly an existing set of
    /// rules.
    fn new(db: Database, rules: Option<MutationRules>) -> RecordKeeper {
        RecordKeeper {
            db: RwLock::new(db),
            rules: RwLock::new(rules.unwrap_or(MutationRules::new())),
            pending_txns: RwLock::new(HashMap::new()),
            record_listeners: Mutex::new(ListenerPool::new()),
            game_listeners: Mutex::new(ListenerPool::new()),
            worker: futures_cpupool::Builder::new().pool_size(1).create(),
            priority_worker: futures_cpupool::Builder::new().pool_size(3).create()
        }
    }

    /// Construct a new RecordKeeper by opening a database. This will create a new database if the
    /// one specified does not exist.
    /// # Warning
    /// Any database which is opened, is assumed to contain data in a certain way, any outside
    /// modifications can cause undefined behavior.
    pub fn open(path: PathBuf, rules: Option<MutationRules>, genesis: (Block, Vec<Txn>)) -> Result<RecordKeeper, Error> {
        info!("Opening a RecordKeeper object with path '{:?}'", path);
        let db = Database::open(path)?;
        let rk: RecordKeeper = Self::new(db, rules);
        
        { // Handle Genesis
            let mut db = rk.db.write();
            if db.is_empty() { // add genesis
                debug!("Loaded DB is empty, adding genesis block...");
                for ref txn in genesis.1 {
                    db.add_txn(txn, genesis.0.timestamp)?;
                }
                db.add_block(&genesis.0)?;
                db.walk_to_head()?;
            } else { // make sure the (correct) genesis block is there
                db.get_block(&genesis.0.calculate_hash())?;
            }
        }

        Ok(rk)
    }

    pub fn get_worker(&self) -> &futures_cpupool::CpuPool {
        &self.worker
    }

    pub fn get_priority_worker(&self) -> &futures_cpupool::CpuPool {
        &self.priority_worker
    }

    /// Use pending transactions to create a new block which can then be added to the network.
    /// The block provided is complete except:
    /// 1. The proof of work/proof of stake mechanism has not been completed
    /// 2. The signature has not been applied to the block
    pub fn create_block(&self) -> Result<Block, Error> {
        let pending_txns = self.pending_txns.read();
        let db = self.db.read();

        let cbh = db.get_current_block_header()?;
        let cbh_h = cbh.calculate_hash();

        let txns: BTreeSet<U256> = pending_txns.keys().cloned().collect();

        let block = Block {
            header: BlockHeader{
                version: 1,
                timestamp: Time::current(),
                shard: if cbh.shard.is_zero() { cbh_h } else { cbh.shard },
                prev: cbh_h,
                merkle_root: Block::calculate_merkle_root(&txns),
                blob: Bin::new(),
                creator: U160_ZERO,
                signature: Bin::new()
            },
            txns
        };

        Ok(block)
    }

    /// Add a new block and its associated transactions to the chain state after verifying
    /// it is valid. Also move the network state to be at the new end of the chain.
    /// Returns true if the block was added, false if it was already in the system.
    pub fn add_block(&self, block: &Block, fresh: bool) -> Result<bool, Error> {
        self.is_valid_block(block)?;

        let mut pending_txns = self.pending_txns.write();
        let mut db = self.db.write();

        // we know it is a valid block, so go ahead and add it's transactions, and then it.
        for txn_hash in block.txns.iter() {
            if let Some((recv_time, txn)) = pending_txns.remove(txn_hash) { // we will need to add it
                db.add_txn(&txn, recv_time)?;
            } else {
                // should already be in the DB then because otherwise is_valid_block should give an
                // error, so use an assert check
                assert!(db.get_txn(*txn_hash).is_ok())
            }
        }
        // add txns first, so that we make sure all blocks in the system have all their information,
        // plus, any txn which is somehow added without its block will probably be added by another
        // anyway so it is the lesser of the evils

        // record the block
        if db.add_block(block)? {
            let hash = block.calculate_hash();

            // move the network state
            let initial_height = db.get_current_block_height();
            let invalidated = db.walk_to_head()?;
            let uncled = hash != db.get_current_block_hash();
            
            // couple of quick checks...
            // if uncled, basic verification that we have not moved
            debug_assert!(!uncled || (
                invalidated == 0 &&
                initial_height == db.get_current_block_height()
            ));
            // if not uncled, do some validity checks to make sure we moved correctly
            debug_assert!(uncled || (
                initial_height > invalidated &&
                initial_height < db.get_current_block_height()
            ));

            // send out events as needed
            let mut record_listeners = self.record_listeners.lock();
            if invalidated > 0 {
                record_listeners.notify(&RecordEvent::StateInvalidated{
                    new_height: db.get_current_block_height(),
                    after_height: initial_height - invalidated
                });
            }
            record_listeners.notify(&RecordEvent::NewBlock{uncled, fresh, block: block.clone()});

            Ok(true)
        } else { // we already knew about this block, do nothing
            Ok(false)
        }
    }

    /// Add a new transaction to the pool of pending transactions after validating it. Returns true
    /// if it was added successfully to pending transactions, and returns false if it is already in
    /// the list of pending transactions or accepted into the database..
    pub fn add_pending_txn(&self, txn: Txn, fresh: bool) -> Result<bool, Error> {
        let hash = txn.calculate_hash();

        let mut txns = self.pending_txns.write();
        let db = self.db.read();

        let pending_size = txns.values()
            .fold(0, |acc, &(_, ref t)| acc + (t.calculate_size()));
        if pending_size + txn.calculate_size() > MAX_PENDING_TXN_MEM {
            return Err(Error::OutOfMemory("Maximum pending txn memory reached.".into()));
        }
        
        // check if it is already pending
        if txns.contains_key(&hash) {
            return Ok(false);
        }

        // check if it is already in the database
        match db.get_txn(hash) {
            Ok(_) => return Ok(false),
            Err(Error::NotFound(..)) => {},
            Err(e) => return Err(e)
        }

        debug!("New pending txn ({})", txn.calculate_hash());

        // add the event
        self.is_valid_txn_given_lock(&*db, &*txns, &txn)?;
        txns.insert(hash, (Time::current(), txn.clone()));

        // notify listeners
        self.record_listeners.lock().notify(&RecordEvent::NewTxn{fresh, txn: txn.clone() });
        let mut game_listeners = self.game_listeners.lock();
        for change in txn.mutation.changes.iter() {  match change {
            &Change::PlotEvent(ref e) => {
                game_listeners.notify(e);
            }
            _ => (),
        }}
        
        Ok(true)
    }

    /// Import a package of blocks and transactions. Returns the hash of the last block imported.
    pub fn import_pkg(&self, pkg: BlockPackage) -> Result<U256, Error> {
        let (blocks, txns) = pkg.unpack();
        debug!("Importing {} blocks and {} txns to database.", blocks.len(), txns.len());

        if blocks.is_empty() {
            // it is invalid to import an empty block package
            return Err(Error::Deserialize("Empty Block Package".into()));
        }

        let last = blocks.last().unwrap().calculate_hash();

        for txn in txns {
            self.add_pending_txn(txn.1, false)?;
        } for block in blocks {
            self.add_block(&block, false)?;
        }
        
        Ok(last)
    }

    /// Find a validator's public key given the hash. If they are not found, then they are not a
    /// validator.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_key(&self, id: &U160) -> Result<Bin, Error> {
        self.db.read()
            .get_validator_key(*id)
    }

    /// Get the reputation of a validator given their ID.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_rep(&self, id: &U160) -> Result<i64, Error> {
        self.db.read()
            .get_validator_rep(*id)
    }

    /// Retrieve the current block hash which the network state represents.
    pub fn get_current_block_hash(&self) -> U256 {
        self.db.read().get_current_block_hash()
    }

    /// Retrieve the header of the current block which the network state represents.
    pub fn get_current_block_header(&self) -> Result<BlockHeader, Error> {
        let db = self.db.read();
        let hash = db.get_current_block_hash();
        db.get_block_header(&hash)
    }

    /// Retrieve the current block which the network state represents.
    pub fn get_current_block(&self) -> Result<Block, Error> {
        let db = self.db.read();
        let hash = db.get_current_block_hash();
        db.get_block(&hash)
    }

    /// Calculate the height of a given block. It will follow the path until it finds the genesis
    /// block which is denoted by having a previous block reference of 0.
    pub fn get_block_height(&self, hash: &U256) -> Result<u64, Error> {
        let db = self.db.read();
        db.get_block_height(*hash)
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    pub fn get_blocks_of_height(&self, height: u64) -> Result<Vec<U256>, Error> {
        let db = self.db.read();
        db.get_blocks_of_height(height)
    }

    /// Get a list of the last `count` block headers. If `count` is one, then it will return only
    /// the most recent block.
    pub fn get_latest_blocks(&self, count: usize) -> Result<Vec<BlockHeader>, Error> {
        let db = self.db.read();
        db.get_latest_blocks(count)
    }
    
    /// This is designed to get blocks between a start and end hash. It will get blocks from
    /// (last_known, target]. Do not include last-known because it is clearly already in the system,
    /// but do include the target block since it has not yet been accepted into the database.
    pub fn get_blocks_between(&self, last_known: &U256, target: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let db = self.db.read();
        debug!("Packaging blocks between {} and {}", last_known, target);
        BlockPackage::blocks_between(&*db, last_known, target, limit)
    }

    /// Returns a map of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `from_tick` simply allows additional filtering, e.g. if
    /// you set `from_tick` to 0, you would not get all events unless the oldest events have not
    /// yet been removed from the cache.
    pub fn get_plot_events(&self, plot_id: PlotID, from_tick: u64) -> Result<RawEvents, Error> {
        let mut events: RawEvents = {
            let db = self.db.read();
            db.get_plot_events(plot_id, from_tick)?
        };
        
        let txns = self.pending_txns.read();
        for &(_, ref txn) in txns.values() {
            for change in &txn.mutation.changes {
                if let &Change::PlotEvent(ref e) = change {
                    if e.tick >= from_tick && (e.from == plot_id) || (e.to.contains(&plot_id)) {
                        event::add_event(&mut events, e.tick, e.event.clone());
                    }
                }
            }
        }
        
        Ok(events)
    }

    /// Add a new listener for events such as new blocks. This will also take a moment to remove any
    /// listeners which no longer exist.
    pub fn register_record_listener(&self, listener: Sender<RecordEvent>) {
        self.record_listeners.lock().register(listener);
    }

    /// Add a new listener for plot events. This will also take a moment to remove any listeners
    /// which no longer exist.
    pub fn register_game_listener(&self, listener: Sender<PlotEvent>) {
        self.game_listeners.lock().register(listener);
    }

    /// Add a new rule to the database regarding what network mutations are valid. This will only
    /// impact things which are mutated through the `mutate` function.
    pub fn add_rule(&mut self, rule: Box<MutationRule>) {
        let mut rules_lock = self.rules.write();
        rules_lock.push_back(rule);
    }

    /// Add a list of new rules to the database regarding what network mutations are valid. These
    /// will only impact things which are mutated through the `mutate` function.
    pub fn add_rules(&mut self, rules: MutationRules) {
        let mut rules_lock = self.rules.write();
        for rule in rules {
            rules_lock.push_back(rule);
        }
    }

    /// Check if a block is valid and all its components.
    pub fn is_valid_block(&self, block: &Block) -> Result<(), Error> {
        let db = self.db.read();
        let state = NetState::new(
            &*db, db.get_diff(&db.get_current_block_hash(), &block.prev)?
        );
        self.is_valid_block_given_state(&state, &db, block)
    }

    /// Check if a txn is valid given the current network state: PlotID, to: &BTreeSet<PlotID>,
    /// tick: u64, event: &RawEventte. Use this to validate pending txns.
    pub fn is_valid_txn(&self, txn: &Txn) -> Result<(), Error> {
        let pending = self.pending_txns.read();
        let db = self.db.read();
        self.is_valid_txn_given_lock(&*db, &*pending, txn)
    }

    /// Retrieve a block header from the database.
    pub fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        let db = self.db.read();
        db.get_block_header(hash)
    }

    /// Get a block including its list of transactions from the database.
    pub fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        let db = self.db.read();
        db.get_block(hash)
    }

    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let db = self.db.read();
        db.complete_block(header)
    }

    /// Get a transaction from the database.
    pub fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let pending = self.pending_txns.read();
        let db = self.db.read();
        match pending.get(&hash) {
            Some(&(_, ref txn)) => Ok(txn.clone()),
            None => db.get_txn(*hash)
        }
    }

    /// Whether or not the block is part of the longest chain, and therefore influences the history
    pub fn is_block_in_current_chain(&self, hash: &U256) -> Result<bool, Error> {
        let db = self.db.read();
        db.is_part_of_current_chain(*hash)
    }

    /// Get the block a txn is part of. It will return None if the txn is found to be pending.
    pub fn get_txn_blocks(&self, txn: U256) -> Result<Option<HashSet<U256>>, Error> {
        // check pending txns
        for (h, _t) in self.pending_txns.read().iter() {
            if *h == txn { return Ok(None) }
        }

        // check DB
        self.db.read().get_txn_blocks(txn).map(|x| Some(x))
    }

    /// Get the txns which were created by a given account.
    pub fn get_account_txns(&self, account: &U160) -> Result<HashSet<U256>, Error> {
        let mut txns = HashSet::new();
        for (txn_hash, &(_, ref txn)) in self.pending_txns.read().iter() {
            if txn.creator == *account { txns.insert(txn_hash.clone()); }
        }
        for txn in self.db.read().get_account_txns(account)? {
            txns.insert(txn);
        } Ok(txns)
    }

    /// Returns the amount of shares associated with an account, a basic operation sometimes required for forging
    /// TODO: Fill in
    pub fn get_account_value(&self, _account: &U160) -> Result<u64, Error> {
        Ok(1)
    }

    /// Get the time a txn was originally received.
    pub fn get_txn_receive_time(&self, txn: U256) -> Result<Time, Error> {
        if let Some(&(time, _)) = self.pending_txns.read().get(&txn) {
            return Ok(time);
        }
        self.db.read().get_txn_receive_time(txn)
    }

    /// Internal use function to check if a block and all its sub-components are valid.
    fn is_valid_block_given_state(&self, state: &NetState, db: &Database, block: &Block) -> Result<(), Error> {
        rules::block::TimeStamp.is_valid(state, db, block)?;
        rules::block::Signature.is_valid(state, db, block)?;
        rules::block::MerkleRoot.is_valid(state, db, block)?;

        let mut mutation = Vec::new();
        for txn_hash in &block.txns {
            let txn = self.get_txn_given_lock(db, &txn_hash)?;
            self.is_valid_txn_given_state(state, &txn)?;
            for change in txn.mutation.changes {
                mutation.push((change, txn.creator));
            }
        }

        self.is_valid_mutation_given_state(state, &mutation)
    }

    /// Check if a txn is valid given access to the database and pending txns. Will construct a
    /// network state.
    fn is_valid_txn_given_lock(&self, db: &Database, pending: &HashMap<U256, (Time, Txn)>, txn: &Txn) -> Result<(), Error> {
        let state = {
            let cur = db.get_current_block_hash();
            let mut diff = NetDiff::new(cur, cur);
            for mutation in pending.values().map(|&(_, ref txn)| txn.mutation.clone()) {
                diff.apply_mutation(mutation);
            }
            NetState::new(&*db, diff)
        };
        self.is_valid_txn_given_state(&state, txn)?;

        let mut mutation = Vec::new();
        for change in txn.mutation.changes.iter().cloned() {
            mutation.push((change, txn.creator));
        }
        self.is_valid_mutation_given_state(&state, &mutation)
    }

    /// Internal use function, check if a txn is valid.
    fn is_valid_txn_given_state(&self, state: &NetState, txn: &Txn) -> Result<(), Error> {
        rules::txn::Signature.is_valid(state, txn)?;
        rules::txn::AdminCheck.is_valid(state, txn)?;
        rules::txn::NewValidator.is_valid(state, txn)?;
        rules::txn::Duplicates.is_valid(state, txn)
    }

    /// Internal use function to check if a mutation is valid.
    fn is_valid_mutation_given_state(&self, state: &NetState, mutation: &Vec<(Change, U160)>) -> Result<(), Error> {
        let mut cache = Bin::new();
        // base rules
        rules::mutation::PlotEvent.is_valid(state, mutation, &mut cache)?;
        rules::mutation::Duplicates.is_valid(state, mutation, &mut cache)?;

        // user-added rules
        cache = Bin::new();
        let rules = self.rules.read();
        for rule in &*rules {
            // verify all rules are satisfied and return, propagate error if not
            rule.is_valid(state, mutation, &mut cache)?;
        }
        Ok(())
    }

    fn get_txn_given_lock(&self, db: &Database, hash: &U256) -> Result<Txn, Error> {
        if let Some(&(_, ref txn)) = self.pending_txns.read().get(hash) {
            Ok(txn.clone())
        } else {
            db.get_txn(*hash)
        }
    }

    pub fn get_stats(&self) -> Result<RecordKeeperStatistics, Error> {
        let current_block = self.get_current_block_hash();

        let ptxns = self.pending_txns.read();

        Ok(RecordKeeperStatistics {
            height: self.get_block_height(&current_block)?,
            current_block_hash: current_block.into(),

            pending_txns_count: ptxns.len() as u64,
            pending_txns_size: ptxns.values().fold(0, |acc, &(_, ref ptxn)| acc + (ptxn.calculate_size() as u64))
        })
    }
}
