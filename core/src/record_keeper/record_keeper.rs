use bin::Bin;
use primitives::{JU256, U256, U160, U160_ZERO, U256_ZERO, Txn, Block, BlockHeader, HasBlockHeader, Change, ListenerPool};
use std::collections::{HashMap, BTreeSet, BTreeMap, HashSet};
use std::path::PathBuf;
use parking_lot::{RwLock, Mutex};
use primitives::{RawEvents, event, Mutation};
use super::{BlockPackage, Error, RecordEvent, PlotEvent, PlotID, DBState, rules, BlockRule, TxnRule, MutationRule, MutationRules, database::*};
use time::Time;

use futures::sync::mpsc::Sender;

const MAX_PENDING_TXN_MEM: usize = 128*1024*1024; //128 MB


#[derive(Debug)]
pub struct RecordKeeperConfig {
    /// Maximum size in bytes of the pending transaction pool before transactions are dropped, one
    /// way or another.
    pub pending_txn_limit: u64,

    /// To what extent should data be stored by record keeper?
    pub index_strategy: RecordKeeperIndexingStrategy,

    /// The custom mutation rules which record keeper should use to validate txns
    pub rules: MutationRules,
}


#[derive(Debug)]
pub enum RecordKeeperIndexingStrategy {
    /// Full indexing capability, including all data needed for particapating in regular blockchain
    /// operations and caches to speed up performace.
    Full,

    /// Minimum number of indexes required to fully validate and participate in regular blockchain
    /// operations
    Standard,

    /// Slimmed down database, not storing all blockchain data, just a minimum amount to be somewhat
    /// informed
    Light
}


#[derive(Debug, Serialize)]
/// RK Stats which can be sent via JSON on request.
pub struct RecordKeeperStatistics {
    pub height: u64,
    pub current_block_hash: JU256,

    pub pending_txns_count: u64,
    pub pending_txns_size: u64,
}

pub trait RecordKeeper: Send + Sync {
    /// Get information about the current status of RK.
    fn get_stats(&self) -> Result<RecordKeeperStatistics, Error> {
        Ok(RecordKeeperStatistics {
            height: 1,
            current_block_hash: U256_ZERO.into(),

            pending_txns_count: 0,
            pending_txns_size: 0
        })
    }

    /// Use pending transactions to create a new block which can then be added to the network.
    /// The block provided is complete except:
    /// 1. The proof of work/proof of stake mechanism has not been completed
    /// 2. The signature has not been applied to the block
    fn create_block(&self) -> Result<Block, Error> {

        let txns: BTreeSet<U256> = BTreeSet::new();

        let block = Block {
            header: BlockHeader {
                version: 1,
                timestamp: Time::current(),
                shard: U256_ZERO,
                prev: U256_ZERO,
                merkle_root: Block::calculate_merkle_root(&txns),
                blob: Bin::new()
            },
            txns
        };

        Ok(block)
    }

    /// Add a new block and its associated transactions to the chain state after verifying
    /// it is valid. Also move the network state to be at the new end of the chain.
    /// Returns true if the block was added, false if it was already in the system.
    fn add_block(&self, _block: &Block, _fresh: bool) -> Result<bool, Error> {
        Ok(true)
    }

    /// Add a new transaction to the pool of pending transactions after validating it. Returns true
    /// if it was added successfully to pending transactions, and returns false if it is already in
    /// the list of pending transactions or accepted into the database..
    fn add_pending_txn(&self, _txn: Txn, _fresh: bool) -> Result<bool, Error> {
        Ok(true)
    }

    /// Import a package of blocks and transactions. Returns the hash of the last block imported.
    fn import_pkg(&self, pkg: BlockPackage) -> Result<U256, Error> {
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
    fn get_validator_key(&self, _id: &U160) -> Result<Bin, Error> {
        Ok(Bin::new())
    }

    /// Get the shares of a validator given their ID.
    /// TODO: Handle shard-based shares
    fn get_validator_stake(&self, _id: &U160) -> Result<u64, Error> {
        Ok(1)
    }

    /// Retrieve the current block hash which the network state represents.
    fn get_current_block_hash(&self) -> U256 {
        U256_ZERO
    }

    /// Retrieve the header of the current block which the network state represents.
    fn get_current_block_header(&self) -> Result<BlockHeader, Error> {
        self.create_block().map(|b| b.get_header().clone())
    }

    /// Retrieve the current block which the network state represents.
    fn get_current_block(&self) -> Result<Block, Error> {
        self.create_block()
    }

    /// Lookup the height of a given block which is in the DB.
    /// *Note:* This requires the block is in the DB already.
    fn get_block_height(&self, _hash: &U256) -> Result<u64, Error> {
        Ok(1)
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    fn get_blocks_of_height(&self, _height: u64) -> Result<Vec<U256>, Error> {
        Ok(Vec::new())
    }

    /// Get a list of the last `count` block headers. If `count` is one, then it will return only
    /// the most recent block.
    fn get_latest_blocks(&self, _count: usize) -> Result<Vec<BlockHeader>, Error> {
        Ok(vec![])
    }

    /// This is designed to get blocks between a start and end hash. It will get blocks from
    /// (last_known, target]. Do not include last-known because it is clearly already in the system,
    /// but do include the target block since it has not yet been accepted into the database.
    fn get_blocks_between(&self, _last_known: &U256, _target: &U256, _limit: usize) -> Result<BlockPackage, Error> {
        Ok(BlockPackage::new_empty())
    }

    /// Returns a map of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `from_tick` simply allows additional filtering, e.g. if
    /// you set `from_tick` to 0, you would not get all events unless the oldest events have not
    /// yet been removed from the cache.
    fn get_plot_events(&self, _plot_id: PlotID, _from_tick: u64) -> Result<RawEvents, Error> {
        Ok(BTreeMap::new())
    }

    /// Add a new listener for events such as new blocks. This will also take a moment to remove any
    /// listeners which no longer exist.
    fn register_record_listener(&self, _listener: Sender<RecordEvent>) {}

    /// Add a new listener for plot events. This will also take a moment to remove any listeners
    /// which no longer exist.
    fn register_game_listener(&self, _listener: Sender<PlotEvent>) {}

    /// Check if a block is valid and all its components.
    fn is_valid_block(&self, _block: &Block) -> Result<(), Error> {
        Ok(())
    }

    /// Check if a txn is valid given the current network state. Use this to validate pending txns,
    /// but do not use if simply going to add the txn as it will check there.
    fn is_valid_txn(&self, _txn: &Txn) -> Result<(), Error> {
        Ok(())
    }

    /// Retrieve a block header from the database.
    fn get_block_header(&self, _hash: &U256) -> Result<BlockHeader, Error> {
        self.get_current_block_header()
    }

    /// Get a block including its list of transactions from the database.
    fn get_block(&self, _hash: &U256) -> Result<Block, Error> {
        self.get_current_block()
    }

    /// Convert a block header into a full block.
    fn complete_block(&self, _header: BlockHeader) -> Result<Block, Error> {
        self.get_current_block()
    }

    /// Get a transaction from the database.
    fn get_txn(&self, _hash: &U256) -> Result<Txn, Error> {
        Ok(Txn::new(U160_ZERO, Mutation::new()))
    }

    /// Whether or not the block is part of the longest chain, and therefore influences the history
    fn is_block_in_current_chain(&self, _hash: &U256) -> Result<bool, Error> {
        Ok(true)
    }

    /// Get the block a txn is part of. It will return None if the txn is found to be pending.
    fn get_txn_blocks(&self, _txn: U256) -> Result<Option<HashSet<U256>>, Error> {
        Ok(None)
    }

    /// Get the txns which were created by a given account.
    fn get_account_txns(&self, _account: &U160) -> Result<HashSet<U256>, Error> {
        Ok(HashSet::new())
    }

    /// Get the time a txn was originally received.
    fn get_txn_receive_time(&self, _txn: U256) -> Result<Time, Error> {
        Ok(Time::from_milliseconds(0))
    }
}


/// An abstraction on the concept of states and state state data. Builds higher-lsuperevel
/// functionality on top of the database. The implementation uses RwLocks to provide many read,
/// single write thread safety.
///
/// TODO: Also add a block to the known blocks if it is only referenced.
/// TODO: Also allow for reaching out to the network to request missing information.
/// TODO: Allow removing state data for shards which are not being processed.
pub struct RecordKeeperImpl<DB> {
    config: RecordKeeperConfig,

    db: RwLock<DB>,
    pending_txns: RwLock<HashMap<U256, (Time, Txn)>>,

    record_listeners: Mutex<ListenerPool<RecordEvent>>,
    game_listeners: Mutex<ListenerPool<PlotEvent>>,

}

impl<DB: Database> RecordKeeper for RecordKeeperImpl<DB> {
    /// Get information about the current status of RK.
    fn get_stats(&self) -> Result<RecordKeeperStatistics, Error> {
        let current_block = self.get_current_block_hash();

        let ptxns = self.pending_txns.read();

        Ok(RecordKeeperStatistics {
            height: self.get_block_height(&current_block)?,
            current_block_hash: current_block.into(),

            pending_txns_count: ptxns.len() as u64,
            pending_txns_size: ptxns.values().fold(0, |acc, &(_, ref ptxn)| acc + (ptxn.calculate_size() as u64))
        })
    }

    /// Use pending transactions to create a new block which can then be added to the network.
    /// The block provided is complete except:
    /// 1. The proof of work/proof of stake mechanism has not been completed
    /// 2. The signature has not been applied to the block
    fn create_block(&self) -> Result<Block, Error> {
        let pending_txns = self.pending_txns.read();
        let db = self.db.read();

        let cbh = db.get_current_block_header()?;
        let cbh_h = cbh.calculate_hash();

        let txns: BTreeSet<U256> = pending_txns.keys().cloned().collect();

        let block = Block {
            header: BlockHeader {
                version: 1,
                timestamp: Time::current(),
                shard: if cbh.shard.is_zero() { cbh_h } else { cbh.shard },
                prev: cbh_h,
                merkle_root: Block::calculate_merkle_root(&txns),
                blob: Bin::new()
            },
            txns
        };

        Ok(block)
    }

    /// Add a new block and its associated transactions to the chain state after verifying
    /// it is valid. Also move the network state to be at the new end of the chain.
    /// Returns true if the block was added, false if it was already in the system.
    fn add_block(&self, block: &Block, fresh: bool) -> Result<bool, Error> {
        let hash = block.calculate_hash();
        if self.get_block(&hash).is_ok() {
            return Ok(false); // block already exists
        }

        self.is_valid_block(block)?;

        let mut pending_txns = self.pending_txns.write();
        let mut db = self.db.write();

        // get around the scope issues
        let initial_height = db.get_current_block_height();
        let (invalidated_blocks, earliest_invalidated_tick, uncled);

        // construct a write batch of all changes we are making using a DBState object.
        let wb = {  // because consuming state with the compile operation is not enough for the borrow checker...
            let mut state = DBState::new(&*db);

            // we know it is a valid block, so go ahead and add it and then its transactions
            if !state.add_block(block)? {
                // between checking if the block was valid and taking out a write lock, the block
                // has already been added.
                return Ok(false);
            }

            for txn_hash in block.txns.iter() {
                if let Some((recv_time, txn)) = pending_txns.remove(txn_hash) { // we will need to add it
                    state.add_txn(&txn, recv_time)?;
                } else {
                    // should already be in the DB then because otherwise is_valid_block should give an
                    // error, so use an assert check
                    assert!(state.get_txn(*txn_hash).is_ok())
                }
            }

            {
                let (a, b) = state.walk_to_head()?;
                invalidated_blocks = a;
                earliest_invalidated_tick = b;
            }

            uncled = hash != state.get_current_block_hash();

            // couple of quick checks...
            // if uncled, basic verification that we have not moved
            debug_assert!(!uncled || (
                invalidated_blocks == 0 &&
                    initial_height == state.get_current_block_height()
            ));
            // if not uncled, do some validity checks to make sure we moved correctly
            debug_assert!(uncled || (
                initial_height > invalidated_blocks &&
                    initial_height < state.get_current_block_height()
            ));

            // write the changes to the actual db
            state.compile()?
        };

        db.apply(wb)?;

        // send out events as needed
        let mut record_listeners = self.record_listeners.lock();
        if invalidated_blocks > 0 {
            record_listeners.notify(&RecordEvent::StateInvalidated {
                new_height: db.get_current_block_height(),
                after_height: initial_height - invalidated_blocks,
                after_tick: earliest_invalidated_tick
            });
        }
        record_listeners.notify(&RecordEvent::NewBlock { uncled, fresh, block: block.clone() });

        Ok(true)
    }

    /// Add a new transaction to the pool of pending transactions after validating it. Returns true
    /// if it was added successfully to pending transactions, and returns false if it is already in
    /// the list of pending transactions or accepted into the database..
    fn add_pending_txn(&self, txn: Txn, fresh: bool) -> Result<bool, Error> {
        let hash = txn.calculate_hash();

        // check if it is already pending or in db
        if self.pending_txns.read().contains_key(&hash) || self.get_txn(&hash).is_ok() {
            return Ok(false);
        }

        let mut pending_txns = self.pending_txns.write();
        let db = self.db.read();

        let pending_size = pending_txns.values()
            .fold(0, |acc, &(_, ref t)| acc + (t.calculate_size()));
        if pending_size + txn.calculate_size() > MAX_PENDING_TXN_MEM {
            return Err(Error::OutOfMemory("Maximum pending txn memory reached.".into()));
        }

        // check if it is already in the database
        match db.get_txn(hash) {
            Ok(_) => return Ok(false),
            Err(Error::NotFound(..)) => {},
            Err(e) => return Err(e)
        }

        debug!("New pending txn ({})", txn.calculate_hash());

        // add the event
        self.is_valid_txn_given_lock(&*db, &*pending_txns, &txn)?;
        pending_txns.insert(hash, (Time::current(), txn.clone()));

        // notify listeners
        self.record_listeners.lock().notify(&RecordEvent::NewTxn { fresh, txn: txn.clone() });
        let mut game_listeners = self.game_listeners.lock();
        for change in txn.mutation.changes.iter() {
            match change {
                &Change::PlotEvent(ref e) => {
                    game_listeners.notify(e);
                }
                _ => (),
            }
        }

        Ok(true)
    }

    /// Find a validator's public key given the hash. If they are not found, then they are not a
    /// validator.
    fn get_validator_key(&self, id: &U160) -> Result<Bin, Error> {
        self.db.read()
            .get_validator_key(*id)
    }

    /// Get the shares of a validator given their ID.
    /// TODO: Handle shard-based shares
    fn get_validator_stake(&self, _id: &U160) -> Result<u64, Error> {
        // temp, come back to this later
        /*self.db.read()
            .get_validator_stake(*id)*/

        Ok(1)
    }

    /// Import a package of blocks and transactions. Returns the hash of the last block imported.
    fn import_pkg(&self, pkg: BlockPackage) -> Result<U256, Error> {
        let time = Time::current();

        // Retrieve the blocks and txns to import
        let (blocks, txns) = pkg.unpack();
        let txns = txns.into_iter()
            .map(|(k, v)| (k, (time, v)))
            .collect::<HashMap<U256, (Time, Txn)>>();
        debug!("Importing {} blocks and {} txns to database.", blocks.len(), txns.len());

        if blocks.is_empty() {
            // it is invalid to import an empty block package
            return Err(Error::Deserialize("Empty Block Package".into()));
        }

        let last = blocks.last().unwrap().calculate_hash();

        // Lock the state as we make verify and plan (and eventually make) changes
        let mut pending_txns = self.pending_txns.write();
        let mut db = self.db.write();

        let initial_height = db.get_current_block_height();
        let initial_block = db.get_current_block_hash();

        // get around scope issues.
        let (invalidated_blocks, earliest_invalidated_tick);

        let wb = {
            let mut state = DBState::new(&*db);

            // Add all blocks and associated transactions and verify they are valid
            for block in blocks.iter() {
                let block_hash = block.calculate_hash();
                if state.get_block_header(&block_hash).is_ok() {
                    // the block has already in the system.
                    continue;
                }

                { // Check if it is valid
                    // Yipee for second level differences!
                    // (I just new making state implement DB would be worthwhile...)
                    // TODO: should we walk our own state forward to reduce how far subsequent blocks have to walk to get to the same place?
                    let prior_block_state = DBState::new(&state).at(block.prev)?;
                    self.is_valid_block_given_state(&prior_block_state, &txns, &block)?;
                }

                let added = state.add_block(&block)?;
                debug_assert!(added); // already verified it was not present

                for txn_hash in block.txns.iter() {
                    let txn = txns.get(txn_hash)
                        .expect("Missing transaction after validating block from block package.");
                    state.add_txn(&txn.1, time)?;
                }

                state.walk_to_head()?;
            }

            let (_undone_block, new_blocks, earliest_tick) =
                state.calculate_invalidations_to_block(&initial_block)?;
            invalidated_blocks = new_blocks;
            earliest_invalidated_tick = earliest_tick;

            state.compile()?
        };

        // Write the changes
        db.apply(wb)?;
        let db = db.downgrade();

        // Validate all pending transactions against the new state
        {
            use std::mem::swap;

            let mut txns = HashMap::with_capacity(pending_txns.len());
            swap(&mut txns, &mut *pending_txns);

            for (txn_hash, (recv_time, txn)) in txns {
                if self.is_valid_txn_given_lock(&*db, &*pending_txns, &txn).is_err() {
                    continue;
                }
                // else
                pending_txns.insert(txn_hash, (recv_time, txn));
            }

            drop(pending_txns);
        }

        // Notify listeners of the changes
        let mut record_listeners = self.record_listeners.lock();

        for block in blocks {
            let uncled = db.is_part_of_current_chain(block.calculate_hash())?;
            record_listeners.notify(&RecordEvent::NewBlock { uncled, fresh: false, block });
        }

        if invalidated_blocks > 0 {
            record_listeners.notify(&RecordEvent::StateInvalidated {
                new_height: db.get_current_block_height(),
                after_height: initial_height - invalidated_blocks,
                after_tick: earliest_invalidated_tick
            });
        }

        Ok(last)
    }

    /// Retrieve the current block hash which the network state represents.
    fn get_current_block_hash(&self) -> U256 {
        self.db.read().get_current_block_hash()
    }

    /// Retrieve the header of the current block which the network state represents.
    fn get_current_block_header(&self) -> Result<BlockHeader, Error> {
        let db = self.db.read();
        let hash = db.get_current_block_hash();
        db.get_block_header(&hash)
    }

    /// Retrieve the current block which the network state represents.
    fn get_current_block(&self) -> Result<Block, Error> {
        let db = self.db.read();
        let hash = db.get_current_block_hash();
        db.get_block(&hash)
    }

    /// Lookup the height of a given block which is in the DB.
    /// *Note:* This requires the block is in the DB already.
    fn get_block_height(&self, hash: &U256) -> Result<u64, Error> {
        let db = self.db.read();
        db.get_block_height(*hash)
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    fn get_blocks_of_height(&self, height: u64) -> Result<Vec<U256>, Error> {
        let db = self.db.read();
        db.get_blocks_of_height(height)
    }

    /// Get a list of the last `count` block headers. If `count` is one, then it will return only
    /// the most recent block.
    fn get_latest_blocks(&self, count: usize) -> Result<Vec<BlockHeader>, Error> {
        let db = self.db.read();
        db.get_latest_blocks(count)
    }

    /// This is designed to get blocks between a start and end hash. It will get blocks from
    /// (last_known, target]. Do not include last-known because it is clearly already in the system,
    /// but do include the target block since it has not yet been accepted into the database.
    fn get_blocks_between(&self, last_known: &U256, target: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let db = self.db.read();
        debug!("Packaging blocks between {} and {}", last_known, target);
        BlockPackage::blocks_between(&*db, last_known, target, limit)
    }

    /// Returns a map of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `from_tick` simply allows additional filtering, e.g. if
    /// you set `from_tick` to 0, you would not get all events unless the oldest events have not
    /// yet been removed from the cache.
    fn get_plot_events(&self, plot_id: PlotID, from_tick: u64) -> Result<RawEvents, Error> {
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
    fn register_record_listener(&self, listener: Sender<RecordEvent>) {
        self.record_listeners.lock().register(listener);
    }

    /// Add a new listener for plot events. This will also take a moment to remove any listeners
    /// which no longer exist.
    fn register_game_listener(&self, listener: Sender<PlotEvent>) {
        self.game_listeners.lock().register(listener);
    }

    /// Check if a block is valid and all its components.
    fn is_valid_block(&self, block: &Block) -> Result<(), Error> {
        let pending = self.pending_txns.read();
        let db = self.db.read();
        let state = DBState::new(&*db).at(block.prev)?;
        self.is_valid_block_given_state(&state, &*pending, block)
    }

    /// Check if a txn is valid given the current network state. Use this to validate pending txns,
    /// but do not use if simply going to add the txn as it will check there.
    fn is_valid_txn(&self, txn: &Txn) -> Result<(), Error> {
        let pending = self.pending_txns.read();
        let db = self.db.read();
        self.is_valid_txn_given_lock(&*db, &*pending, txn)
    }

    /// Retrieve a block header from the database.
    fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        let db = self.db.read();
        db.get_block_header(hash)
    }

    /// Get a block including its list of transactions from the database.
    fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        let db = self.db.read();
        db.get_block(hash)
    }

    /// Convert a block header into a full block.
    fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let db = self.db.read();
        db.complete_block(header)
    }

    /// Get a transaction from the database.
    fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let pending = self.pending_txns.read();
        let db = self.db.read();
        match pending.get(&hash) {
            Some(&(_, ref txn)) => Ok(txn.clone()),
            None => db.get_txn(*hash)
        }
    }

    /// Whether or not the block is part of the longest chain, and therefore influences the history
    fn is_block_in_current_chain(&self, hash: &U256) -> Result<bool, Error> {
        let db = self.db.read();
        db.is_part_of_current_chain(*hash)
    }

    /// Get the block a txn is part of. It will return None if the txn is found to be pending.
    fn get_txn_blocks(&self, txn: U256) -> Result<Option<HashSet<U256>>, Error> {
        // check pending txns
        for (h, _t) in self.pending_txns.read().iter() {
            if *h == txn { return Ok(None) }
        }

        // check DB
        self.db.read().get_txn_blocks(txn).map(|x| Some(x))
    }

    /// Get the txns which were created by a given account.
    fn get_account_txns(&self, account: &U160) -> Result<HashSet<U256>, Error> {
        let mut txns = HashSet::new();
        for (txn_hash, &(_, ref txn)) in self.pending_txns.read().iter() {
            if txn.creator == *account { txns.insert(txn_hash.clone()); }
        }
        for txn in self.db.read().get_account_txns(account)? {
            txns.insert(txn);
        }
        Ok(txns)
    }

    /// Get the time a txn was originally received.
    fn get_txn_receive_time(&self, txn: U256) -> Result<Time, Error> {
        if let Some(&(time, _)) = self.pending_txns.read().get(&txn) {
            return Ok(time);
        }
        self.db.read().get_txn_receive_time(txn)
    }
}


impl RecordKeeperImpl<DatabaseImpl> {
    /// Construct a new RecordKeeper by opening a database. This will create a new database if the
    /// one specified does not exist.
    /// # Warning
    /// Any database which is opened, is assumed to contain data in a certain way, any outside
    /// modifications can cause undefined behavior.
    pub fn open(path: PathBuf, config: RecordKeeperConfig, genesis: (Block, Vec<Txn>)) -> Result<Self, Error> {
        info!("Opening a RecordKeeper object with path '{:?}'", path);
        let db = DatabaseImpl::open(path)?;
        let rk = Self::new(db, config);

        { // Handle Genesis
            let mut db = rk.db.write();
            let wb = if db.is_empty() { // add genesis
                debug!("Loaded DB is empty, adding genesis block...");
                let mut state = DBState::new(&*db);
                for ref txn in genesis.1 {
                    state.add_txn(txn, genesis.0.timestamp)?;
                }
                state.add_block(&genesis.0)?;
                state.walk_to_head()?;

                Some(state.compile()?)
            } else { // make sure the (correct) genesis block is there
                db.get_block(&genesis.0.calculate_hash())?;
                None
            };

            if let Some(wb) = wb {
                db.apply(wb)?;
            }
        }

        Ok(rk)
    }
}


impl<DB: Database> RecordKeeperImpl<DB> {
    /// Construct a new RecordKeeper from an already opened database and possibly an existing set of
    /// rules.
    fn new(db: DB, config: RecordKeeperConfig) -> RecordKeeperImpl<DB> {
        RecordKeeperImpl {
            config: config,
            db: RwLock::new(db),
            pending_txns: RwLock::new(HashMap::new()),
            record_listeners: Mutex::new(ListenerPool::new()),
            game_listeners: Mutex::new(ListenerPool::new()),
        }
    }

    /// Internal use function to check if a block and all its sub-components are valid.
    fn is_valid_block_given_state(&self, prev_block_state: &DBState, pending: &HashMap<U256, (Time, Txn)>, block: &Block) -> Result<(), Error> {
        rules::block::TimeStamp.is_valid(prev_block_state, block)?;
        rules::block::MerkleRoot.is_valid(prev_block_state, block)?;

        let mut mutation = Vec::new();
        for txn_hash in &block.txns {
            let txn = pending.get(txn_hash)
                .map(|t| Ok(t.1.clone()))
                .unwrap_or_else(|| prev_block_state.get_txn(*txn_hash))?;

            self.is_valid_txn_given_state(prev_block_state, &txn)?;
            for change in txn.mutation.changes {
                mutation.push((change, txn.creator));
            }
        }

        // verifies all txns are valid together
        self.is_valid_mutation_given_state(prev_block_state, &mutation)
    }

    /// Check if a txn is valid given access to the database and pending txns. Will construct a
    /// DBState with all txns applied.
    fn is_valid_txn_given_lock(&self, db: &dyn Database, pending: &HashMap<U256, (Time, Txn)>, txn: &Txn) -> Result<(), Error> {
        let state = DBState::new(db);

        self.is_valid_txn_given_state(&state, txn)?;
        
        // make one big mutation. if the mutation is invalid, then the new txn is what caused it to be invalid
        let mut mutation = Vec::new();
        for (_, txn) in pending.values() {
            for change in txn.mutation.changes.iter().cloned() {
                mutation.push((change, txn.creator));
            }
        }
        for change in txn.mutation.changes.iter().cloned() {
            mutation.push((change, txn.creator));
        }
        self.is_valid_mutation_given_state(&state, &mutation)
    }

    /// Internal use function, check if a txn is valid.
    fn is_valid_txn_given_state(&self, state: &DBState, txn: &Txn) -> Result<(), Error> {
        rules::txn::Signature.is_valid(state, txn)?;
        rules::txn::AdminCheck.is_valid(state, txn)?;
        rules::txn::NewValidator.is_valid(state, txn)?;
        rules::txn::Duplicates.is_valid(state, txn)
    }

    /// Internal use function to check if a mutation is valid.
    fn is_valid_mutation_given_state(&self, prev_block_state: &DBState, mutation: &Vec<(Change, U160)>) -> Result<(), Error> {
        let mut cache = Bin::new();
        // base rules
        rules::mutation::PlotEvent.is_valid(prev_block_state, mutation, &mut cache)?;
        rules::mutation::Duplicates.is_valid(prev_block_state, mutation, &mut cache)?;

        // user-added rules
        cache = Bin::new();
        let rules = &self.config.rules;
        for rule in &*rules {
            // verify all rules are satisfied and return, propagate error if not
            rule.is_valid(prev_block_state, mutation, &mut cache)?;
        }
        Ok(())
    }
}
