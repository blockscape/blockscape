use bin::Bin;
use primitives::{U256, U160, U256_ZERO, Txn, Block, BlockHeader, HasBlockHeader};
use std::collections::{HashMap, BTreeSet, BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::{RwLock,Mutex};
use futures_cpupool;
use primitives::{RawEvents};
use super::{RecordKeeper, PoolHolder, RecordKeeperStatistics, Error, LogicError, PlotID};
use super::BlockPackage;
use time::Time;

#[derive(Debug)]
struct BlockTreeNode {
    pub block: Block,
    pub height: u64,
    pub children: Mutex<Vec<Arc<BlockTreeNode>>>
}

/// A record keeper which maintains an in-memory database of blocks and transactions
pub struct DummyRecordKeeper {
    blocks: Arc<BlockTreeNode>,
    blocks_hashes: RwLock<HashMap<U256, Arc<BlockTreeNode>>>,
    txns: RwLock<HashMap<U256, Txn>>,
    pending_txns: RwLock<HashMap<U256, Txn>>,

    best_block: RwLock<Arc<BlockTreeNode>>,
}

impl DummyRecordKeeper {
    pub fn new() -> DummyRecordKeeper {

        let node = Arc::new(BlockTreeNode {
            block: Block {
                header: BlockHeader {
                    version: 1,
                    timestamp: Time::from_milliseconds(0),
                    shard: U256_ZERO,
                    prev: U256_ZERO,
                    merkle_root: Block::calculate_merkle_root(&BTreeSet::new()),
                    blob: Bin::new()
                },
                txns: BTreeSet::new()
            },
            height: 1,
            children: Mutex::new(Vec::new())
        });

        let mut bhs = HashMap::new();

        bhs.insert(node.block.calculate_hash(), node.clone());

        DummyRecordKeeper {
            blocks: node.clone(),
            blocks_hashes: RwLock::new(bhs),
            txns: RwLock::new(HashMap::new()),
            pending_txns: RwLock::new(HashMap::new()),
            best_block: RwLock::new(node),
        }
    }
}

/// TODO: Find a way to optimize out
impl PoolHolder for DummyRecordKeeper {
    fn get_worker(&self) -> &futures_cpupool::CpuPool {
        panic!("Should not be using the worker for dummy RK!")
    }

    /// Get the CPU pool worker for high-priority tasks.
    fn get_priority_worker(&self) -> &futures_cpupool::CpuPool {
        panic!("Should not be using the worker for dummy RK!")
    }
}

impl RecordKeeper for DummyRecordKeeper {
        /// Get information about the current status of RK.
    fn get_stats(&self) -> Result<RecordKeeperStatistics, Error> {
        let current_block = self.get_current_block_hash();

        Ok(RecordKeeperStatistics {
            height: self.get_block_height(&current_block)?,
            current_block_hash: current_block.into(),

            pending_txns_count: self.pending_txns.read().unwrap().len() as u64,
            pending_txns_size: self.pending_txns.read().unwrap().values().fold(0, |acc, ref ptxn| acc + (ptxn.calculate_size() as u64))
        })
    }

    /// Use pending transactions to create a new block which can then be added to the network.
    /// The block provided is complete except:
    /// 1. The proof of work/proof of stake mechanism has not been completed
    /// 2. The signature has not been applied to the block
    fn create_block(&self) -> Result<Block, Error> {

        let cbh = self.get_current_block_header()?;
        let cbh_h = cbh.calculate_hash();

        let txns: BTreeSet<U256> = self.pending_txns.read().unwrap().keys().cloned().collect();

        let block = Block {
            header: BlockHeader {
                version: 1,
                timestamp: Time::current(),
                shard: U256_ZERO,
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
    fn add_block(&self, block: &Block, _fresh: bool) -> Result<bool, Error> {

        if let Some(_) = self.blocks_hashes.read().unwrap().get(&block.calculate_hash()) {
            return Ok(false);
        }

        let bh = self.blocks_hashes.read().unwrap().get(&block.prev).cloned();

        if let Some(node) = bh {
            
            let mut ptxns = self.pending_txns.write().unwrap();
            let mut atxns = self.txns.write().unwrap();

            // first, we must have all the transactions listed within the block in our db
            for txn in block.txns.iter() {
                if ptxns.get(&txn).is_none() {
                    return Err(Error::Logic(LogicError::MissingPrevious));
                }
            }

            // now we can start adding the block
            for txn in block.txns.iter() {
                atxns.insert(*txn, ptxns.remove(txn).unwrap());
            }

            let h = block.calculate_hash();
            let new_node = Arc::new(BlockTreeNode {
                block: block.clone(),
                height: node.height + 1,
                children: Mutex::new(Vec::new())
            });

            {
                let mut childs = node.children.lock().unwrap();
                childs.push(Arc::clone(&new_node));
            }
            {
                let mut bb = self.best_block.write().unwrap();
                if bb.height < new_node.height {
                    *bb = Arc::clone(&new_node);
                }
            }

            self.blocks_hashes.write().unwrap().insert(h, new_node); 

            Ok(true)
        }
        else {
            Err(Error::Logic(LogicError::MissingPrevious))
        }
    }

    /// Add a new transaction to the pool of pending transactions after validating it. Returns true
    /// if it was added successfully to pending transactions, and returns false if it is already in
    /// the list of pending transactions or accepted into the database..
    fn add_pending_txn(&self, txn: Txn, _fresh: bool) -> Result<bool, Error> {
        let hash = txn.calculate_hash();
        Ok(self.pending_txns.write().unwrap().insert(hash, txn).is_none())
    }

    /// Get the shares of a validator given their ID.
    /// TODO: Handle shard-based shares
    fn get_validator_stake(&self, _id: &U160) -> Result<u64, Error> {
        Ok(1)
    }

    /// Retrieve the current block hash which the network state represents.
    fn get_current_block_hash(&self) -> U256 {
        self.best_block.read().unwrap().block.calculate_hash()
    }

    /// Retrieve the header of the current block which the network state represents.
    fn get_current_block_header(&self) -> Result<BlockHeader, Error> {
        Ok(self.best_block.read().unwrap().block.get_header().clone())
    }

    /// Retrieve the current block which the network state represents.
    fn get_current_block(&self) -> Result<Block, Error> {
        Ok(self.best_block.read().unwrap().block.clone())
    }

    /// Lookup the height of a given block which is in the DB.
    /// *Note:* This requires the block is in the DB already.
    fn get_block_height(&self, hash: &U256) -> Result<u64, Error> {
        self.blocks_hashes.read().unwrap().get(hash)
            .map(|n| n.height).ok_or(Error::Deserialize("Could not find".into()))
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    /*fn get_blocks_of_height(&self, height: u64) -> Result<Vec<U256>, Error> {
        let db = self.db.read();
        db.get_blocks_of_height(height)
    }*/

    /// Get a list of the last `count` block headers. If `count` is one, then it will return only
    /// the most recent block.
    fn get_latest_blocks(&self, count: usize) -> Result<Vec<BlockHeader>, Error> {

        let mut blocks = Vec::new();

        let mut cur = self.get_current_block_hash();
        for _ in 0..count {
            let b = self.get_block_header(&cur)?;
            cur = b.prev;
            
            blocks.push(b);
        }

        Ok(blocks)
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
    fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        if let Some(node) = self.blocks_hashes.read().unwrap().get(hash) {
            Ok(node.block.get_header().clone())
        }
        else {
            Err(Error::Deserialize("Could not find block header!".into()))
        }
    }

    /// Get a block including its list of transactions from the database.
    fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        if let Some(node) = self.blocks_hashes.read().unwrap().get(hash) {
            Ok(node.block.clone())
        }
        else {
            Err(Error::Deserialize("Could not find block header!".into()))
        }
    }

    /// Convert a block header into a full block.
    fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        self.get_block(&header.calculate_hash())
    }

    /// Get a transaction from the database.
    fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        if let Some(txn) = self.txns.write().unwrap().get(hash) {
            Ok(txn.clone())
        }
        else if let Some(txn) = self.pending_txns.read().unwrap().get(hash) {
            Ok(txn.clone())
        }
        else {
            Err(Error::Deserialize("Could not find txn hash!".into()))
        }
    }

    /// Whether or not the block is part of the longest chain, and therefore influences the history
    fn is_block_in_current_chain(&self, hash: &U256) -> Result<bool, Error> {

        // make sure the block exists at all first!
        self.get_block(hash)?;

        // we basically have to walk backwards
        let mut cur = self.get_current_block_hash();
        while cur != U256_ZERO && cur != *hash {
            cur = self.get_block(&cur)?.prev;
        }

        Ok(cur == *hash)
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
    fn get_txn_receive_time(&self, txn: U256) -> Result<Time, Error> {
        self.get_txn(&txn).map(|txn| txn.timestamp)
    }
}

#[test]
fn block_creation() {
    let rk = DummyRecordKeeper::new();

    // test add a block to root
    let b = rk.create_block().expect("Should be able to create block");
    let hash = b.calculate_hash();
    assert_eq!(rk.add_block(&b, true).unwrap(), true);
    assert_eq!(rk.add_block(&b, true).unwrap(), false);
    assert_eq!(rk.get_current_block_hash(), hash);
    assert_eq!(rk.get_block_height(&hash).unwrap(), 2);

    // test add block to current chain    
    let mut b2 = rk.create_block().expect("Should be able to create block");
    let hash2 = b2.calculate_hash();
    assert_eq!(rk.add_block(&b2, true).unwrap(), true);
    assert_eq!(rk.get_current_block_hash(), hash2);
    assert_eq!(rk.get_block_height(&hash2).unwrap(), 3);

    // test adding block to nonexistant block
    b2.header.prev = U256::from(1);
    rk.add_block(&b2, true).expect_err("Should fail to add block with nonexistant previous hash");

    // test adding block to older block in chain
    b2.header.prev = b.prev;
    assert!(rk.add_block(&b2, true).unwrap());
    assert_eq!(rk.get_current_block_hash(), hash2);
    assert_eq!(rk.get_stats().expect("Should get statistics").height, 3);

    // test getting block by hash
    assert_eq!(rk.get_block(&hash).unwrap(), b);
}

#[test]
fn txns_in_block() {

    use primitives::Mutation;

    let rk = DummyRecordKeeper::new();

    // test adding txn to pending pool
    let txn = Txn::new(U160::from(0), Mutation::new());
    assert!(rk.add_pending_txn(txn.clone(), true).unwrap());
    assert!(!rk.add_pending_txn(txn.clone(), true).unwrap());
    assert_eq!(rk.get_stats().expect("Should get statistics").pending_txns_count, 1);

    // test pending txn is added to created block
    let b = rk.create_block().expect("Should be able to create block");
    rk.add_block(&b, true).expect("Should be able to add block");
    assert_eq!(rk.get_stats().expect("Should get statistics").pending_txns_count, 0);
    assert_eq!(rk.get_current_block().expect("Should have a current block").txns.iter().next().expect("Should have txn in block"), &txn.calculate_hash());

    // test adding txn to pending pool still works
    let txn2 = Txn::new(U160::from(0), Mutation::new());
    assert!(rk.add_pending_txn(txn2.clone(), true).unwrap());
    assert_eq!(rk.get_stats().expect("Should get statistics").pending_txns_count, 1);

    // test querying txns both in the pending and not pending pool
    assert_eq!(rk.get_txn(&txn.calculate_hash()).unwrap(), txn);
    assert_eq!(rk.get_txn(&txn2.calculate_hash()).unwrap(), txn2);
}