use bin::{Bin, AsBin};
use bincode;
use hash::hash_pub_key;
use primitives::{U256, U160, Mutation, Change, Block, BlockHeader, Txn, RawEvent, RawEvents};
use primitives::event;
use rocksdb::{DB, Options, IteratorMode, DBCompressionType};
use rocksdb::Error as RocksDBError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet, BTreeMap};
use std::path::PathBuf;
use super::{PlotID, NetDiff, PlotEvent};
use super::error::*;
use super::key::*;
use num_cpus;


/// The reward bestowed for backing the correct block
pub const BLOCK_REWARD: i64 = 10;
/// The number of ticks grouped together into a "bucket" within the network state.
pub const PLOT_EVENT_BUCKET_SIZE: u64 = 1000;

/// Represents the current head of the blockchain
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct HeadRef {
    pub block: U256,
    pub height: u64
}

impl Default for HeadRef {
    fn default() -> HeadRef {
        use primitives::u256::U256_ZERO;
        HeadRef{block: U256_ZERO, height: 0}
    }
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
/// TODO: Convert this to Shard and split of Network State?
pub struct Database {
    db: DB,
    head: HeadRef
}


impl Database {    
    /// Create a new Database from a RocksDB instance
    fn new(db: DB) -> Database {
        let head = //attempt to read the current block
            if let Ok(value) = Self::get_raw_data_static(&db, CacheEntry::CurrentHead.into()) {
                bincode::deserialize(&value).unwrap_or(HeadRef::default())
            } else { HeadRef::default() };

        Database{ db, head }
    }

    /// Open the RocksDB database based on the environment or by the given path. Construct a new
    /// Database by opening an existing one or creating a new database if the one specified does not
    /// exist.
    /// # Warning
    /// Any database which is opened, is assumed to contain data in a certain way, any outside
    /// modifications can cause undefined behavior.
    pub fn open(path: PathBuf) -> Result<Database, RocksDBError> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.set_compression_type(DBCompressionType::Lz4hc);
        options.increase_parallelism(num_cpus::get() as i32);
        Ok(
            DB::open_default(path)
            .map(|db| Self::new(db))?
        )
    }

    pub fn is_empty(&self) -> bool {
        self.db.iterator(IteratorMode::Start).next().is_none()
    }

    /// Retrieve raw data from the database. Use this for non-storable types (mostly network stuff).
    #[inline]
    pub fn get_raw_data(&self, key: Key) -> Result<Bin, Error> {
        Self::get_raw_data_static(&self.db, key)
    }

    fn get_raw_data_static(db: &DB, key: Key) -> Result<Bin, Error> {
        // let res = db.get(&key.as_bin())?
        //     .map(|d| d.to_vec())
        //     .ok_or(Error::NotFound(key.clone()));
        // println!("GET {:?}: {:?}", key, res); res

        db.get(&key.as_bin())?
            .map(|d| d.to_vec())
            .ok_or(Error::NotFound(key))
    }

    pub fn get<S: DeserializeOwned>(&self, key: Key) -> Result<S, Error> {
        let raw = self.get_raw_data(key)?;
        Ok(bincode::deserialize(&raw)?)
    }

    /// Put raw data into the database. Should have no uses outside this class.
    #[inline]
    pub fn put_raw_data(&mut self, key: Key, data: &[u8]) -> Result<(), Error> {
        Self::put_raw_data_static(&self.db, key, data)
    }

    fn put_raw_data_static(db: &DB, key: Key, data: &[u8]) -> Result<(), Error> {
        // println!("PUT {:?}: {:?}", key, data);
        Ok(db.put(&key.as_bin(), &data)?)
    }

    pub fn put<S: Serialize>(&mut self, key: Key, object: &S, size: Option<u64>) -> Result<(), Error> {
        let raw = match size {
            Some(s) => bincode::serialize(object, bincode::Bounded(s)).unwrap(),
            None => bincode::serialize(object, bincode::Infinite).unwrap()
        };

        self.put_raw_data(key, &raw)
    }

    /// Add a new transaction to the database.
    #[inline]
    pub fn add_txn(&mut self, txn: &Txn) -> Result<(), Error> {
        let hash = txn.calculate_hash();
        debug!("Adding txn ({}) to database", &hash);
        self.put(BlockchainEntry::Txn(hash).into(), txn, None)?;
        self.add_txn_to_account(txn.creator, hash)
    }

    /// Retrieve a block header form the database given a hash.
    pub fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        self.get(BlockchainEntry::BlockHeader(*hash).into())
    }

    /// Adds a block to the database and records it in the height cache.
    /// Returns true if the block was added, and false if it was already in the system.
    pub fn add_block(&mut self, block: &Block) -> Result<bool, Error> {
        let hash = block.calculate_hash();
        debug!("Adding block ({}) to database containing {} txns.", hash, block.txns.len());

        if self.get_block_header(&hash).is_ok() { return Ok(false) }

        // put the header in the db
        self.put(BlockchainEntry::BlockHeader(hash).into(), &block.header, None)?;
        // put txns into db under the merkle root
        self.put(BlockchainEntry::TxnList(block.merkle_root).into(), &block.txns, None)?;
        
        // add the block to the height cache
        let height = self.get_block_height(block.prev)? + 1;
        
        // cache the block
        self.add_block_to_height(height, &hash)?;
        self.add_height_for_block(height, hash)?;
        for txn in block.txns.iter() {
            self.add_block_for_txn(*txn, hash)?;
        }

        Ok(true)
    }

    /// Retrieve an entire block object from the database given a hash.
    pub fn get_block(&self, hash: &U256) -> Result<Block, Error> {
        // Blocks are stored with their header separate from the transaction body, so get the header
        // first to find the merkle_root, and then get the list of transactions and piece them
        // together.
        let header = self.get_block_header(hash)?;
        self.complete_block(header)
    }

    /// Get the hash of the current head of the blockchain as it lines up with the network state.
    /// That is, the current head is that which the network state represents.
    #[inline]
    pub fn get_current_block_hash(&self) -> U256 {
        self.head.block
    }

    /// Get the header of the current block of the blockchain as it lines up with the network state.
    #[inline]
    pub fn get_current_block_header(&self) -> Result<BlockHeader, Error> {
        self.get_block_header(&self.head.block)
    }

    /// Get the height of the current head of the blockchain.
    #[inline]
    pub fn get_current_block_height(&self) -> u64 {
        self.head.height
    }

    /// Retrieve the transactions for a block to complete a `BlockHeader` as a `Block` object.
    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let txns = self.get(BlockchainEntry::TxnList(header.merkle_root).into())?;
        Ok(Block{header, txns})
    }

    /// Get a transaction that has been recorded in the database. It will only be recorded in the DB
    /// if it was accepted in a block. Said block may be an uncle.
    pub fn get_txn(&self, hash: U256) -> Result<Txn, Error> {
        self.get(BlockchainEntry::Txn(hash).into())
    }

    /// Get the block(s) a txn is part of.
    pub fn get_txn_blocks(&self, hash: U256) -> Result<HashSet<U256>, Error> {
        let blocks: HashSet<U256> = self.get(CacheEntry::BlocksByTxn(hash).into())?;
        Ok(blocks)

        // Scanning method
        /*// verify we know of the txn so it is not a wild goose chase, will return NotFound error if
        // it is not in the DB.
        self.get_txn(hash)?;

        // now find where it is
        for res in DownIter(&self, self.get_current_block_hash()) {
            let (b_hash, header) = res?;
            let block = self.complete_block(header)?;
            for h in block.txns {
                if h == hash {
                    return Ok(b_hash);
                }
            }
        }

        unreachable!() */
    }

    /// Get the txns created by a given account.
    pub fn get_account_txns(&self, hash: U160) -> Result<HashSet<U256>, Error> {
        let txns = self.get(CacheEntry::TxnsByAccount(hash).into())?;
        Ok(txns)
    }

    /// Get the public key of a validator given their ID.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_key(&self, id: U160) -> Result<Bin, Error> {
        self.get_raw_data(NetworkEntry::ValidatorKey(id).into())
    }

    /// Get the reputation of a validator given their ID.
    /// TODO: Handle shard-based reputations
    #[inline]
    pub fn get_validator_rep(&self, id: U160) -> Result<i64, Error> {
        self.get(NetworkEntry::ValidatorRep(id).into())
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    pub fn get_blocks_of_height(&self, height: u64) -> Result<Vec<U256>, Error> {
        let key = CacheEntry::BlocksByHeight(height).into();
        map_not_found(self.get(key), Vec::new())
    }

    /// Retrieve the block which is part of the current chain at a given height.
    pub fn get_current_block_of_height(&self, height: u64) -> Result<U256, Error> {
        let key = CacheEntry::BlocksByHeight(height).into();
        Ok(self.get::<Vec<U256>>(key)?[0])
    }

    /// Check if a block is part of the current chain, that is, check if it is a direct ancestor of
    /// the current block.
    pub fn is_part_of_current_chain(&self, hash: U256) -> Result<bool, Error> {
        let height = self.get_block_height(hash)?;  // height of block in question
        let member = self.get_current_block_of_height(height)?;  // member of current chain at that height
        Ok(hash == member)  // part of chain iff it is the member at that height
    }



    /// Get the cached height of an existing block.
    pub fn get_block_height(&self, hash: U256) -> Result<u64, Error> {
        if hash == self.head.block { return Ok(self.head.height); }
        self.get(CacheEntry::HeightByBlock(hash).into())
    }


    /// Get a list of the last `count` block headers. If `count` is one, then it will return only
    /// the most recent block.
    pub fn get_latest_blocks(&self, count: usize) -> Result<Vec<BlockHeader>, Error> {
        let mut iter = DownIter(&self, self.head.block).take(count);
        let mut headers = Vec::new();

        while let Some(r) = iter.next() {
            let (_, h) = r?;
            headers.push(h);
        }
        
        Ok(headers)
    }

    /// Get blocks before the `target` hash until it collides with the main chain. If the
    /// `last_known` hash lies between the target and the main chain, it will return the blocks
    /// between them, otherwise it will return the blocks from the main chain until target in that
    /// order and it will not include the last_known block.
    ///
    /// If the limit is reached, it will prioritize blocks of a lower height, but may have a gap
    /// between the main chain (or start) and what it includes.
    fn get_blocks_before(&self, target: &U256, last_known: &U256, limit: usize)  -> Result<Vec<BlockHeader>, Error> {
        let mut iter = DownIter(&self, *target)
            .take(limit)
            .take_while(Result::is_ok)
            .map(Result::unwrap);
        
        // expand this as a loop to allow better error handling.
        let mut blocks = Vec::new();
        while let Some((hash, header)) = iter.next() {
            let in_cur_chain = self.is_part_of_current_chain(hash)?;
            if (hash == *last_known) || (in_cur_chain) { break; }
            blocks.push(header)
        }
        
        blocks.reverse();
        Ok(blocks)
    }

    /// Retrieves all blocks of the current chain which are a descendent of the latest common
    /// ancestor of the last_known block until it reaches target or the end of the known chain. It
    /// will not include the last_known block. Also, `limit` is the maximum number of blocks it
    /// should scan through when ascending the blockchain.
    fn get_blocks_after(&self, last_known: &U256, target: &U256, limit: usize) -> Result<Vec<BlockHeader>, Error> {
        // For efficiency, use a quick check to find if a given block is part of the current chain or not.
        let ancestor_height = {
            let ancestor = self.latest_common_ancestor_with_current_chain(last_known)?;
            self.get_block_height(ancestor)?
        };

        let mut iter = UpIter(&self, ancestor_height)
            .skip(1) // we know they have must have the LCA
            .take(limit) // hard-coded maximum
            .take_while(Result::is_ok) // stop at first error
            .map(Result::unwrap);  // extract block header
        
        let mut blocks = Vec::new();
        while let Some((hash, header)) = iter.next() {
            blocks.push(header);
            if hash == *target { break; }  // stop after adding the target block
        }

        Ok(blocks)
    }

    /// This is designed to get blocks between a start and end hash. It will get blocks from
    /// (last_known, target]. Do not include last-known because it is clearly already in the system,
    /// but do include the target block since it has not yet been accepted into the database.
    pub fn get_blocks_between(&self, last_known: &U256, target: &U256, limit: usize) -> Result<Vec<BlockHeader>, Error> {
        if self.is_part_of_current_chain(*target)? {
            println!("Target ({}) is part of the current chain.", last_known);
            let chain = self.get_blocks_after(last_known, target, limit)?;
            println!("Found {} main chain blocks", chain.len());
            Ok(chain)
        } else {
            println!("Target ({}) is NOT part of the current chain.", last_known);
            let mut uncle_blocks = self.get_blocks_before(target, last_known, limit)?;
            println!("Found {} uncle blocks", uncle_blocks.len());

            let mc_target = uncle_blocks.get(0).map(BlockHeader::calculate_hash).unwrap_or(*target);
            println!("Main chain target: {}", mc_target);
            let mc_limit = limit - uncle_blocks.len();
            let mut main_chain = self.get_blocks_after(last_known, &mc_target, mc_limit)?;
            println!("Found {} main chain blocks", main_chain.len());
            main_chain.append(&mut uncle_blocks);
            Ok(main_chain)
        }
    }

    /// Will find the current head of the blockchain. This uses the last known head to find the
    /// current one by using its block height and searching for ones of a greater height. If two
    /// blocks have the same height, it will choose the last head if it is of the maximum height, or
    /// it will pick randomly from those which are of the greatest known height.
    /// Note: this will need to be updated to support sharding.
    pub fn find_chain_head(&self) -> Result<U256, Error> {
        let mut height = self.head.height;
        let mut choice = self.head.block;

        loop {
            height += 1;

            let blocks = self.get_blocks_of_height(height)?;
            if blocks.is_empty() { return Ok(choice); }  // End when we reach beyond what we know
            choice = blocks[0];
        }
    }

    /// Find the path between `a_block` and `b_block` along the blockchain and return the blocks
    /// sorted by height to get to the main chain, and then to go back up to `b_block`.
    /// Specifically, the first part of the tuple is the sequence of blocks down to the latest
    /// common ancestor, and the second is the blocks up to `b_block` from the latest common
    /// ancestor.
    pub fn calculate_block_path(&self, a_block: &U256, b_block: &U256) -> Result<(BTreeMap<u64, U256>, BTreeMap<u64, U256>), Error> {
        { // verify that we are not crossing shards within reason
            let a_head = self.get_block_header(a_block)?;
            let b_head = self.get_block_header(b_block)?;
            assert!(a_head.shard == b_head.shard || a_head.shard.is_zero() || b_head.shard.is_zero());
        }

        let (a_hashes, b_hashes, last_a, last_b) =
                self.latest_common_ancestor(a_block, b_block)?;
        let (a_dist, b_dist) = Self::intersect_dist(&a_hashes, &b_hashes, &last_a, &last_b);

        let a_height = self.head.height;
        let b_height = a_height - a_dist + b_dist;
        
        // create lists of the `a` and `b` chains sorted by height
        let a_heights: BTreeMap<u64, U256> = a_hashes.into_iter()
            .filter(|&(_, d)| d < a_dist)  // keep only values before intersection
            .map(|(k, d)| {
                assert!(a_height >= d);
                (a_height - d, k)
        }).collect();

        let b_heights: BTreeMap<u64, U256> = b_hashes.into_iter()
            .filter(|&(_, d)| d < b_dist)  // keep only values before intersection
            .map(|(k, d)| {
                assert!(b_height >= d);
                (b_height - d, k)
        }).collect();

        Ok((a_heights, b_heights))
    }

    /// Calculate the changes needed to move the network state from `a_block` to `b_block`. This
    /// walks the network state and creates a Diff object of the changes. To walk backwards on the
    /// chain it requires use of contra transactions, so the `a_block` must be either come before
    /// `b_block` or be on the main chain to work.
    pub fn get_diff(&self, a_block: &U256, b_block: &U256) -> Result<NetDiff, Error> {
        let (a_heights, b_heights) = self.calculate_block_path(a_block, b_block)?;
        
        // construct the diff
        let mut diff = NetDiff::new(*a_block, *b_block);
        // go down `a` chain and then go up `b` chain.
        for (h, b) in a_heights.into_iter().rev() {
            debug_assert!(h > 1);
            diff.apply_contra(self.get_contra(b)?);
        }
        for (h, b) in b_heights {
            debug_assert!(h > 1);
            let block = self.get_block(&b)?;
            diff.apply_mutation(self.get_mutation(&block)?);
        } Ok(diff)
    }

    /// Walk the network state to a given block in the block chain. Returns the number of blocks
    /// which were invalidated in the walking process (if any). E.g., if it returns 5, then the 5
    /// latest blocks were undone and are no longer part of the network state.
    pub fn walk(&mut self, b_block: &U256) -> Result<u64, Error> {
        let a_block = self.head.block;
        if a_block == *b_block { return Ok(0); }
        debug!("Walking the network state from {} to {}.", a_block, b_block);
        assert!(!b_block.is_zero(), "Cannot walk to nothing");

        // TODO: We may only need to have a count of the number of blocks to undo when walking backwards
        let (a_heights, b_heights) = self.calculate_block_path(&a_block, b_block)?;
        debug_assert!(a_heights.len() < b_heights.len());
        
        // the number of blocks invalidated is equal to the number of blocks we are going to undo.
        let invalidated_blocks = a_heights.len() as u64;

        // go down `a` chain and then go up `b` chain.
        for (h, b) in a_heights.into_iter().rev() {
            debug_assert!(h > 1);
            debug_assert!(self.head.block == b);
            let header = self.get_block_header(&b)?;
            let contra = self.get_contra(b)?;
            self.undo_mutate(contra)?;
            self.update_current_block(header.prev, Some(h - 1))?;
        }
        for (h, b) in b_heights {
            debug_assert!(h > 1);
            let block = self.get_block(&b)?;
            debug_assert!(block.prev == self.head.block);
            let mutation = self.get_mutation(&block)?;
            let contra = self.mutate(&mutation)?;
            self.add_contra(b, &contra)?;
            self.update_current_chain(h, &b)?;
            self.update_current_block(b, Some(h))?;
        }
        
        Ok(invalidated_blocks)
    }

    /// Find the current head of the block chain and then walk to it. Returns the number of blocks
    /// which were invalidated in the walking process (if any). E.g., if it returns 5, then the 5
    /// latest blocks were undone and are no longer part of the network state.
    #[inline]
    pub fn walk_to_head(&mut self) -> Result<u64, Error> {
        if self.head.block.is_zero() { // walk from nothingness to genesis block
            let blocks = self.get_blocks_of_height(1)?;
            assert_eq!(blocks.len(), 1); // should have exactly one entry if in genesis case
            let genesis = blocks[0];
            let block = self.get_block(&genesis)?;
            let mutation = self.get_mutation(&block)?;
            self.mutate(&mutation)?; // don't need contra for the genesis block
            // or to update current chain since there is only one block
            self.update_current_block(genesis, Some(1))?;
            Ok(0)
        } else { // normal case
            let head = self.find_chain_head()?;
            self.walk(&head)
        }
    }

    /// Returns a map of events for each tick that happened after a given tick. Note: it will not
    /// seek to reconstruct old history so `from_tick` simply allows additional filtering, e.g. if
    /// you set `from_tick` to 0, you would not get all events unless the oldest events have not
    /// yet been removed from the cache.
    pub fn get_plot_events(&self, plot_id: PlotID, from_tick: u64) -> Result<RawEvents, Error> {
        let mut tick = from_tick;
        let mut event_list = RawEvents::new();
        loop {
            let key: Key = NetworkEntry::Plot(plot_id, tick).into();
            
            match self.get::<RawEvents>(key.clone()) {
                Ok(mut l) => event_list.append(&mut l),
                Err(Error::NotFound(..)) => break,
                Err(e) => return Err(e)
            }
            
            tick += PLOT_EVENT_BUCKET_SIZE;
        } Ok(event_list.split_off(&from_tick))
    }

    /// Put together a mutation object from all of the individual transactions
    pub fn get_block_mutation(&self, block: &Block) -> Result<Mutation, Error> {
        let mut mutation = Mutation::new();
        for txn_hash in &block.txns {
            let txn = self.get_txn(*txn_hash)?;
            mutation.merge_clone(&txn.mutation);
        }
        Ok(mutation)
    }

    /// Set a value in the network state and return the old value if any. It will delete the key
    /// from the database if value is None.
    fn set_value(&mut self, key: Bin, value: &Option<Bin>) -> Result<Option<Bin>, Error> {
        let db_key = Key::Network(NetworkEntry::Generic(key)).as_bin();
        let prior = self.db.get(&db_key)?.map(|v| v.to_vec());

        if let Some(ref v) = *value { // set the value if it is some
            self.db.put(&db_key, v)?;
        } else if prior.is_some() { // otherwise delete it if there was a value to delete
            self.db.delete(&db_key)?
        } Ok(prior)
    }

    /// Change a validator's reputation by the amount indicated.
    fn change_validator_rep(&mut self, id: U160, amount: i64) -> Result<(), Error> {
        let db_key: Key = NetworkEntry::ValidatorRep(id).into();
        let value = map_not_found(self.get::<i64>(db_key.clone()), 0)? + amount;
        self.put(db_key, &value, Some(8))
    }


    
    /// Add a new event to the specified plot.
    fn add_event(&mut self, plot_id: PlotID, tick: u64, event: &RawEvent) -> Result<(), Error> {
        let db_key: Key = NetworkEntry::Plot(plot_id, tick).into();
        let mut event_list = match self.get(db_key.clone()) {
            Ok(list) => list,
            Err(Error::NotFound(..)) => {
                // we need to add empty lists too all prior buckets to make them contiguous
                self.init_event_buckets(plot_id, tick)?;
                RawEvents::new()
            },
            Err(e) => return Err(e)
        };

        event::add_event(&mut event_list, tick, event.clone());
        self.put(db_key, &event_list, None)
    }

    /// Add an event to all the specified plots (iff they are in this shard).
    /// TODO: verify if a PlotID is in the shard
    fn add_events(&mut self, e: &PlotEvent) -> Result<(), Error> {
        self.add_event(e.from, e.tick, &e.event)?;
        for plot in e.to.iter() {
            self.add_event(*plot, e.tick, &e.event)?;
        } Ok(())
    }

    /// Remove an event from a plot. Should only be used when undoing a mutation.
    fn remove_event(&mut self, id: PlotID, tick: u64, event: &RawEvent) -> Result<(), Error> {
        let db_key: Key = NetworkEntry::Plot(id, tick).into();
        match self.get::<RawEvents>(db_key.clone()) {
            Ok(mut event_list) => {
                if !event::remove_event(&mut event_list, tick, event) {
                    warn!("Unable to remove event because it does not exist! The network state \
                        may be desynchronized.");
                } else { self.put(db_key, &event_list, None)?; }
                Ok(())
            },
            Err(Error::NotFound(..)) => { warn!("Unable to remove event because it does not exist! \
                                               The network state may be desynchronized."); Ok(()) },
            Err(e) => Err(e)
        }
    }

    /// Remove an event from all the specified plots (iff they are in this shard).
    /// TODO: verify if a PlotID is in the shard
    fn remove_events(&mut self, e: &PlotEvent) -> Result<(), Error> {
        self.remove_event(e.from, e.tick, &e.event)?;
        for plot in e.to.iter() {
            self.remove_event(*plot, e.tick, &e.event)?;
        } Ok(())
    }


    /// Mutate the stored **network state** and return a contra mutation to be able to undo what was
    /// done. Note that changes to either blockchain state or gamestate must occur through other
    /// functions.
    fn mutate(&mut self, mutation: &Mutation) -> Result<Mutation, Error> {
        mutation.assert_not_contra();
        let mut contra = Mutation::new_contra();

        // for all changes, make the described change and add a contra change for it
        for change in &mutation.changes {   contra.changes.push( match change {
            &Change::Admin{ref key, ref value} => {
                let prior = self.set_value(key.clone(), value)?;
                Change::Admin{key: key.clone(), value: prior}
            },
            &Change::BlockReward{id, ..} => {
                self.change_validator_rep(id, BLOCK_REWARD)?;
                Change::BlockReward{id, proof: Bin::new()}
            },
            &Change::PlotEvent(ref e) => {
                self.add_events(e)?;
                Change::PlotEvent(e.clone())
            },
            &Change::NewValidator{ref pub_key, ..} => {
                let id = hash_pub_key(pub_key);
                let key = NetworkEntry::ValidatorKey(id).into();
                self.put_raw_data(key, pub_key)?;
                Change::NewValidator{pub_key: pub_key.clone()}
            },
            &Change::Slash{id, amount, ..} => {
                self.change_validator_rep(id, -(amount as i64))?;
                Change::Slash{id, amount, proof: Bin::new()}
            }
        })}

        contra.changes.reverse(); // contra goes in reverse of original actions
        Ok(contra)
    }

    /// Consumes a contra mutation to undo changes made by the corresponding mutation to the
    /// network state.
    fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), Error> {
        mutation.assert_contra();

        // For all changes, undo the described action with the data provided
        for change in mutation.changes { match change {
            Change::Admin{key, value} => { self.set_value(key, &value)?; },
            Change::BlockReward{id, ..} => { self.change_validator_rep(id, -BLOCK_REWARD)?; },
            Change::PlotEvent(e) => { self.remove_events(&e)?; },
            Change::NewValidator{pub_key, ..} => {
                let id = hash_pub_key(&pub_key);
                let key: Key = NetworkEntry::ValidatorKey(id).into();
                self.db.delete(&key.as_bin())?;
            },
            Change::Slash{id, amount, ..} => { self.change_validator_rep(id, (amount as i64))?; }
        }}

        Ok(())
    }

    /// Find the latest common ancestor of the block and the head chain.
    /// `a` represents the blocks and their heights we have discovered by descending from the
    /// current head, and `b` represents the blocks discovered from descending form the new hash.
    /// If this succeeds, it returns the path maps of the search, and the `last_a` and `last_b`
    /// values which can be used to interpret the result.
    fn latest_common_ancestor(&self, hash_a: &U256, hash_b: &U256) ->
        Result<(HashMap<U256, u64>, HashMap<U256, u64>, U256, U256), Error>
    {
        // The height listed in `a` is the true height of that block, and the height listed in `b`
        // is its distance from the block in question.
        let mut a: HashMap<U256, u64> = HashMap::new();
        let mut b: HashMap<U256, u64> = HashMap::new();
        
        // insert the starting blocks with a distance from themselves of zero
        a.insert(*hash_a, 0);
        b.insert(*hash_b, 0);
        
        // the last hash added for both chains
        let mut last_a: U256 = *hash_a;
        let mut last_b: U256 = *hash_b;

        // Current running distance from the block in question
        let mut dist: u64 = 1;

        // The goal is to traverse the last blocks in each of them until one of them collides with
        // the other, at that point we can calculate the true height. Technically, we are running
        // until the intersection of `a` and `b` is nonempty, but to save on computation simply
        // check if the new value is in either of them.
        while !a.contains_key(&last_b) &&
              !b.contains_key(&last_a) &&
              !(last_a.is_zero() && last_b.is_zero())
        { // extend each search by 1.
            if !last_a.is_zero() {
                let cur_a = self.get_block_header(&last_a)?.prev;
                a.insert(cur_a, dist);
                last_a = cur_a;
            }
            if !last_b.is_zero() {
                let cur_b = self.get_block_header(&last_b)?.prev;
                b.insert(cur_b, dist);
                last_b = cur_b;
            }
            dist += 1;
        }

        Ok((a, b, last_a, last_b))
    }

    /// Finds the latest common ancestor of a given block and the current block. Traces back along
    /// the chain of the given block until it finds a block it knows is part of the current chain.
    /// Much more efficient than `latest_common_ancestor` in this specific use case.
    fn latest_common_ancestor_with_current_chain(&self, hash: &U256) -> Result<U256, Error> {
        let mut iter = DownIter(&self, *hash);
        while let Some(r) = iter.next() {
            // check if r is part of the main chain, if it is, we are done
            let h: U256 = r?.0;
            if self.is_part_of_current_chain(h)? {
                return Ok(h);
            }
        }
        unreachable!()
    }

    /// Add a block to the blocks by height cache. That way when trying to find all blocks of a
    /// given height, it will be listed.
    fn add_block_to_height(&mut self, height: u64, hash: &U256) -> Result<(), Error> {
        let mut height_vals = self.get_blocks_of_height(height)?;
        if height_vals.contains(hash) { return Ok(()); }
        height_vals.push(*hash);

        self.put(CacheEntry::BlocksByHeight(height).into(), &height_vals, None)
    }

    /// Update the head reference and save it to the database. This should be used when the network
    /// state is changed to represent the current block the state is at.
    fn update_current_block(&mut self, hash: U256, height: Option<u64>) -> Result<(), Error> {
        let h = { // set the height value if it does not exist
            if let Some(h) = height { h }
            else { self.get_block_height(hash)? }
        };

        debug!("Updating Current Head to ({}) of height {}.", hash, h);

        let href = HeadRef{height: h, block: hash};
        self.head = href.clone();
        self.put(CacheEntry::CurrentHead.into(), &href, Some(40))
    }

    /// Used when walking, this moves a given block the front of the list of blocks for the height
    /// which indicates that it is part of the current chain.
    fn update_current_chain(&mut self, height: u64, hash: &U256) -> Result<(), Error> {
        let key: Key = CacheEntry::BlocksByHeight(height).into();
        let mut height_values: Vec<U256> = self.get(key.clone())?;

        if let Some(index) =
            height_values.iter()
            .position(|h| *h == *hash)
        { // we found the one we want, now swap it with the one in the front and re-save it
            height_values.swap(0, index);
            self.put(key, &height_values, None)
        }
        else { // It was not in the list
            Err(Error::NotFound(key))
        }
    }

    /// Cache the height of a block so it can be easily looked up later on.
    fn add_height_for_block(&mut self, height: u64, block: U256) -> Result<(), Error> {
        self.put(CacheEntry::HeightByBlock(block).into(), &height, Some(8))
    }

    /// Add a block to the list of blocks containing a given txn.
    fn add_block_for_txn(&mut self, txn: U256, block: U256) -> Result<(), Error> {
        let mut blocks = map_not_found(self.get_txn_blocks(txn), HashSet::new())?;
        if blocks.insert(block) {
            self.put(CacheEntry::BlocksByTxn(txn).into(), &blocks, None)
        } else { Ok(()) }
    }

    /// Register a txn to a validator account.
    /// TODO: We will likely need to add some sort of bucket system to this eventually.
    fn add_txn_to_account(&mut self, account: U160, txn: U256) -> Result<(), Error> {
        let mut txns: HashSet<U256> = map_not_found(self.get_account_txns(account), HashSet::new())?;
        if txns.insert(txn) {
            self.put(CacheEntry::BlocksByTxn(txn).into(), &txns, None)
        } else { Ok(()) }
    }

    /// Construct a mutation given a block and its transactions by querying the DB for the txns and
    /// then merging their mutations.
    fn get_mutation(&self, block: &Block) -> Result<Mutation, Error> {
        let mut mutation = Mutation::new();
        for txn_h in &block.txns {
            let txn = self.get_txn(*txn_h)?;
            mutation.merge(txn.mutation);
        }
        Ok(mutation)
    }

    /// Retrieve the contra from the db to undo the given block
    fn get_contra(&self, hash: U256) -> Result<Mutation, Error> {
        self.get(CacheEntry::ContraMut(hash).into())
    }

    /// Add a contra for a given block
    fn add_contra(&mut self, hash: U256, contra: &Mutation) -> Result<(), Error> {
        self.put(CacheEntry::ContraMut(hash).into(), contra, None)
    }

    /// Create empty buckets for all ticks before a given point. It will stop when it reaches an
    /// existing bucket or when it has reached the last bucket (at 0).
    fn init_event_buckets(&mut self, plot_id: PlotID, before_tick: u64) -> Result<(), Error> {
        if before_tick < PLOT_EVENT_BUCKET_SIZE { return Ok(()); }
        let mut tick = before_tick - PLOT_EVENT_BUCKET_SIZE; // only want prior buckets.
        loop {
            let key: Key = NetworkEntry::Plot(plot_id, tick).into();
            
            match self.get::<RawEvents>(key.clone()) {
                Ok(..) => break,
                Err(Error::NotFound(..)) => self.put(key, &RawEvents::new(), None)?,
                Err(e) => return Err(e)
            }
            
            if tick < PLOT_EVENT_BUCKET_SIZE { break; }
            tick -= PLOT_EVENT_BUCKET_SIZE;
        } Ok(())
    }

    /// Get the distance of the intersection for the LCA on both paths. Returns
    /// (distance on path a, distance on path b)
    /// Note, this assumes there is a single element which is a member of both `a` and `b`.
    fn intersect_dist(a: &HashMap<U256, u64>, b: &HashMap<U256, u64>, last_a: &U256, last_b: &U256) -> (u64, u64) {
        ({ //distance down `a` path to collision
            if let Some(&d) = a.get(&last_b) { d } // b collided with a
            else { *a.get(&last_a).unwrap() } // last added block was collision
        },{ //distance down `b` path to collision
            if let Some(&d) = b.get(&last_a) { d }  // a collided with b
            else { *b.get(&last_b).unwrap() }  // last added block was collision
        })
    }
}



/// Iterate up the current chain, it will only follow the current chain and will end when either it
/// reaches the head, a database error occurs, or a block header is not found for a block we know is
/// part of the current chain.
pub struct UpIter<'a> (&'a Database, u64);

impl<'a> Iterator for UpIter<'a> {
    type Item = Result<(U256, BlockHeader), Error>;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.1 <= self.0.get_current_block_height() {
            let next = self.0.get_current_block_of_height(self.1);
            if next.is_err() { return Some(Err( next.unwrap_err() )); }

            let header = self.0.get_block_header(next.as_ref().unwrap());
            if header.is_ok() {
                self.1 += 1;
                Some( Ok((next.unwrap(), header.unwrap())) )
            } else {
                Some(Err(header.unwrap_err()))
            }
        } else { None }
    }
}


/// Iterate down a given chain, it will follow the `prev` references provided by `BlockHeader`s.
/// This will end either when it reaches genesis, a database error occurs, or a block header is not
/// found for a block we know comes before it.
pub struct DownIter<'a> (&'a Database, U256);

impl<'a> Iterator for DownIter<'a> {
    type Item = Result<(U256, BlockHeader), Error>;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.1.is_zero() { return None; }
        let res = self.0.get_block_header(&self.1);
        let t = self.1;
        if let Ok(header) = res {
            self.1 = header.prev;
            Some(Ok( (t, header) ))
        } else {
            Some(Err( res.unwrap_err() ))
        }
    }
}
