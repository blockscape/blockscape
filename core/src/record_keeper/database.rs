use bin::Bin;
use bincode;
use env;
use primitives::{U256, U160, Mutation, Change, Block, BlockHeader, Txn};
use rocksdb::{DB, Options};
use rocksdb::Error as RocksDBError;
use std::collections::{HashMap, HashSet, BTreeMap};
use std::path::PathBuf;
use std::rc::Rc;
use super::{Storable, PlotEvent, PlotEvents, events, PlotID};
use super::error::*;

pub const BLOCKCHAIN_POSTFIX: &[u8] = b"b";
pub const CACHE_POSTFIX: &[u8] = b"c";
pub const NETWORK_POSTFIX: &[u8] = b"n";


//--- CACHE STATE ---//
/// A plot and all its associated events
pub const PLOT_PREFIX: &[u8] = b"PLT";
/// A validator's full public key
pub const VALIDATOR_PREFIX: &[u8] = b"VAL";
/// Reputation of a validator (how trustworthy they have proven to be)
pub const REPUTATION_PREFIX: &[u8] = b"REP";

//--- NETWORK STATE ---//
/// All blocks of a given height
pub const BLOCKS_BY_HEIGHT_PREFIX: &[u8] = b"HGT";
/// The height of a given block
pub const HEIGHT_BY_BLOCK_PREFIX: &[u8] = b"BHT";
/// A contra transaction for a block
pub const CONTRA_PREFIX: &[u8] = b"CMT";


/// Key for the current head block used when initializing.
pub const CURRENT_BLOCK: &[u8] = b"CURblock";


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


type EventSet = HashSet<Rc<PlotEvent>>;
type PlotDiffEvents = HashMap<PlotID, EventSet>;

/// A set of changes which define the difference from a given network state to another though
/// walking the blockchain from one point to another. This should be used to compile a list of
/// changes to the network state without having to write to the same place in the DB multiple times.
/// This is designed to be like a diff, so if an event is added but it had been marked as deleted,
/// then it will simply remove it from the list of deleted under the assumption that the net change
/// should be zero.
pub struct NetDiff {
    /// The initial block this is changing from
    pub from: U256,
    /// The block all these changes lead to (if applied to the initial block)
    pub to: U256,
    /// New key-value sets to be added (or overwritten)
    values: HashMap<Bin, Bin>,
    /// Keys which are to be removed from the DB
    delete: HashSet<Bin>,
    /// Events which need to be added to plots
    new_events: PlotDiffEvents,
    /// Events which need to be removed from plots
    removed_events: PlotDiffEvents
}

impl NetDiff {
    fn new(from: U256, to: U256) -> NetDiff {
        NetDiff {
            from, to,
            values: HashMap::new(),
            delete: HashSet::new(),
            new_events: HashMap::new(),
            removed_events: HashMap::new()
        }
    }

    /// Attempt to remove an event from list and return whether it was was there or not.
    fn _remove(plots: &mut HashMap<PlotID, EventSet>, event: &Rc<PlotEvent>) -> bool {
        if // check if it in the `from` plot
            if let Some(plot_a) = plots.get_mut(&event.from) {
                plot_a.remove(event)
            } else { false } // did not remove because plot is not listed
        { //it is, so remove from the `to` plot if unique
            if event.from != event.to {
                let plot_b = plots.get_mut(&event.to)
                        .expect("Events to be registered to both from and to plots");
                plot_b.remove(event);
            } true
        } else { false }
    }

    fn _add(plots: &mut HashMap<PlotID, EventSet>, event: Rc<PlotEvent>) {
        // add first to the `from` plot
        if // check if we need to create a new entry (if not go ahead and append it)
            if let Some(plot_a) = plots.get_mut(&event.from) {
                if plot_a.contains(&event) {return;}
                plot_a.insert(event.clone());
                false
            } else { true }
        { // insert a new entry
            let mut plot_a = HashSet::new();
            plot_a.insert(event.clone());
            plots.insert(event.from, plot_a);
        }

        // check if we need to handle the `to` plot and handle it if so
        if event.from == event.to {return;}
        if // check if we need to create a new entry (if not go ahead and append it)
            if let Some(plot_b) = plots.get_mut(&event.to) {
                let not_already_present = plot_b.insert(event.clone());
                assert!(not_already_present);
                false
            } else { true }
        { // insert new entry
            let mut plot_b = HashSet::new();
            plot_b.insert(event.clone());
            plots.insert(event.to, plot_b);
        }
    }

    /// Add an event to the appropriate plots
    fn add_event(&mut self, event: PlotEvent) {
        let event = Rc::new(event);

        //if it was in removed events, then we don't need to add it
        if !Self::_remove(&mut self.removed_events, &event) {
            Self::_add(&mut self.new_events, event);
        }
    }

    /// Remove an event from the appropriate plots (or mark it to be removed).
    fn remove_event(&mut self, event: PlotEvent) {
        let event = Rc::new(event);

        //if it was in new events events, then we don't need to add it be removed
        if !Self::_remove(&mut self.new_events, &event) {
            Self::_add(&mut self.removed_events, event)
        }
    }

    /// Mark a value to be updated at a given key.
    fn set_value(&mut self, key: Bin, value: Bin) {
        self.delete.remove(&key);
        self.values.insert(key, value);
    }

    /// Mark a key and its value to be removed from the state.
    fn delete_value(&mut self, key: Bin) {
        self.values.remove(&key);
        self.delete.insert(key);
    }

    /// Retrieve the value if any changes have been specified to it. Will return none if no changes
    /// are recorded or if it is to be deleted.
    pub fn get_value(&self, key: &Bin) -> Option<&Bin> {
        self.values.get(key)
    }

    /// Returns whether or not a given value is marked for deletion.
    pub fn is_value_deleted(&self, key: &Bin) -> bool {
        self.delete.contains(key)
    }

    /// Retrieve a list of new events for a given plot.
    pub fn get_new_events(&self, plot: &PlotID) -> Option<&EventSet> {
        self.new_events.get(plot)
    }

    /// Retrieves a list of events to be removed from a given plot.
    pub fn get_removed_events(&self, plot: &PlotID) -> Option<&EventSet> {
        self.removed_events.get(plot) //TODO: avoid cloning?
    }

    /// Check if an event has been marked for removal from its associated plots.
    pub fn is_event_removed(&self, event: &PlotEvent) -> bool {
        if let Some(plot) = self.removed_events.get(&event.from) {
            plot.contains(event)
        } else { false }
    }

    /// Get an iterator over each Plot we have information on and give a list of all things to
    /// remove for it and all things to add to it. See `EventDiffIter`.
    pub fn get_event_changes<'a>(&'a self) -> EventDiffIter {
        let keys = {
            let added: HashSet<_> = self.new_events.keys().cloned().collect();
            let removed: HashSet<_> = self.removed_events.keys().cloned().collect();
            added.union(&removed).cloned().collect::<Vec<_>>()
        };
        
        EventDiffIter(self, keys.into_iter())
    }

    /// Get an iterator over each key we have information on and return if it is deleted or the new
    /// value it should be set to. See `ValueDiffIter`.
    pub fn get_value_changes<'a>(&'a self) -> ValueDiffIter {
        let keys: Vec<&'a Bin> = {
            let added: HashSet<_> = self.values.keys().collect();
            let removed: HashSet<_> = self.delete.iter().collect();
            added.union(&removed).cloned().collect()
        };

        ValueDiffIter(self, keys.into_iter())
    }
}

use std::vec::IntoIter as VecIntoIter;

/// Iterate over all plots we have event changes to make to. The first value is the key, the next is
/// the list of events to remove, and finally it has the list of new events,
pub struct EventDiffIter<'a>(&'a NetDiff, VecIntoIter<PlotID>);
impl<'a> Iterator for EventDiffIter<'a> {
    type Item = (PlotID, Option<&'a EventSet>, Option<&'a EventSet>);

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|k| (k, self.0.get_removed_events(&k), self.0.get_new_events(&k)) )
    }
}

/// Iterate over all values we have changes recorded for. The first part of the Item is the key, and
/// the second part is the value, if the value is None, then the key should be deleted from the DB.
pub struct ValueDiffIter<'a>(&'a NetDiff, VecIntoIter<&'a Bin>);
impl<'a> Iterator for ValueDiffIter<'a> {
    type Item = (&'a Bin, Option<&'a Bin>);

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|k| {
            (k, self.0.get_value(k))
        })
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
    pub fn new(db: DB) -> Database {
        let head = //attempt to read the current block
            if let Ok(value) = Self::get_raw_data_static(&db, CURRENT_BLOCK, CACHE_POSTFIX) {
                bincode::deserialize(&value).unwrap_or(HeadRef::default())
            } else { HeadRef::default() };

        Database{ db, head }
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
    #[inline]
    pub fn get_raw_data(&self, key: &[u8], postfix: &'static [u8]) -> Result<Bin, Error> {
        Self::get_raw_data_static(&self.db, key, postfix)
    }

    fn get_raw_data_static(db: &DB, key: &[u8], postfix: &'static [u8]) -> Result<Bin, Error> {
        let key = Self::with_postfix(key, postfix);

        db.get(&key)?
            .map(|d| d.to_vec())
            .ok_or(Error::NotFound(postfix, Vec::from(key)))
    }

    /// Put raw data into the database. Should have no uses outside this class.
    #[inline]
    pub fn put_raw_data(&mut self, key: &[u8], data: &[u8], postfix: &'static [u8]) -> Result<(), Error> {
        Self::put_raw_data_static(&self.db, key, data, postfix)
    }

    fn put_raw_data_static(db: &DB, key: &[u8], data: &[u8], postfix: &'static [u8]) -> Result<(), Error> {
        let key = Self::with_postfix(key, postfix);
        Ok(db.put(&key, &data)?)
    }

    /// Retrieve and deserialize data from the database. This will return an error if the database
    /// has an issue, if the data cannot be deserialized or if the object is not present in the
    /// database. Note that `instance_id` should be the object's ID/key which would normally be
    /// returned from calling `storable.instance_id()`.
    pub fn get<S: Storable>(&self, instance_id: &[u8], postfix: &'static [u8]) -> Result<S, Error> {
        let key = Self::with_postfix(instance_id, postfix);

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

    /// Add a new transaction to the database.
    #[inline]
    pub fn add_txn(&mut self, txn: &Txn) -> Result<(), Error> {
        self.put(txn, BLOCKCHAIN_POSTFIX)
    }

    /// Retrieve a block header form the database given a hash.
    pub fn get_block_header(&self, hash: &U256) -> Result<BlockHeader, Error> {
        let id = hash.to_vec();
        self.get::<BlockHeader>(&id, BLOCKCHAIN_POSTFIX)
    }

    /// Adds a block to the database and records it in the height cache.
    /// Returns true if the block was added, and false if it was already in the system.
    pub fn add_block(&mut self, block: &Block) -> Result<bool, Error> {
        let hash = block.calculate_hash();

        if self.get_block_header(&hash).is_ok() { return Ok(false) }

        // put the header in the db
        self.put(&block.header, BLOCKCHAIN_POSTFIX)?;

        // put the transactions into the system
        let merkle_root = block.header.merkle_root.to_vec();
        let raw_txns = bincode::serialize(&block.txns, bincode::Infinite)
                .expect("Error serilizing transactions!");
        self.put_raw_data(&merkle_root, &raw_txns, BLOCKCHAIN_POSTFIX)?;
        
        // add the block to the height cache
        let hash = block.calculate_hash();
        let height = self.get_new_block_height(&block.header)?;
        self.cache_block(height, &hash)?;
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

    /// Update the head ref and save it to the database
    pub fn update_current_block(&mut self, hash: &U256, height: Option<u64>) -> Result<(), Error> {
        let h = { // set the height value if it does not exist
            if let Some(h) = height { h }
            else { self.get_block_height(&hash)? }
        };

        self.head.height = h;
        self.head.block = *hash;

        let raw: Vec<u8> = bincode::serialize(&self.head, bincode::Bounded(264)).unwrap();
        self.put_raw_data(CURRENT_BLOCK, &raw, CACHE_POSTFIX)?;

        Ok(())
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

    /// Retrieve the transactions for a block to complete a `BlockHeader` as a `Block` object.
    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let merkle_root = header.merkle_root.to_vec();
        let raw_txns = self.get_raw_data(&merkle_root, BLOCKCHAIN_POSTFIX)?;
        Ok(Block::deserialize(header, &raw_txns)?)
    }

    /// Get a transaction that has been recorded in the database. It will only be recorded in the DB
    /// if it was accepted in a block. Said block may be an uncle.
    pub fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let id = hash.to_vec();
        self.get::<Txn>(&id, BLOCKCHAIN_POSTFIX)
    }

    /// Get the public key of a validator given their ID.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_key(&self, id: &U160) -> Result<Bin, Error> {
        let key = Self::with_prefix(VALIDATOR_PREFIX, &id.to_vec());
        self.get_raw_data(&key, NETWORK_POSTFIX)
    }

    /// Get the reputation of a validator given their ID.
    /// TODO: Handle shard-based reputations
    pub fn get_validator_rep(&self, id: &U160) -> Result<i64, Error> {
        let key = Self::with_prefix(REPUTATION_PREFIX, &id.to_vec());
        let raw = self.get_raw_data(&key, NETWORK_POSTFIX)?;
        Ok(bincode::deserialize::<i64>(&raw)?)
    }

    /// Return a list of **known** blocks which have a given height. If the block has not been added
    /// to the database, then it will not be included.
    pub fn get_blocks_of_height(&self, height: u64) -> Result<Vec<U256>, Error> {
        let key = Database::get_blocks_by_height_key(height);
        let res = self.get_raw_data(&key, CACHE_POSTFIX);
        match res {
            Ok(raw) => { // found something, deserialize
                Ok(bincode::deserialize::<Vec<U256>>(&raw)?)
            },
            Err(e) => match e {
                Error::NotFound(..) => // nothing known to us, so emptyset
                    Ok(Vec::new()),
                _ => Err(e) // some sort of database error
            }
        }
    }

    /// Retrieve the block which is part of the current chain at a given height.
    pub fn get_current_block_of_height(&self, height: u64) -> Result<U256, Error> {
        let key = Database::get_blocks_by_height_key(height);
        let raw = self.get_raw_data(&key, CACHE_POSTFIX)?;
        let list: Vec<U256> = bincode::deserialize(&raw).unwrap();
        Ok(list[0])
    }

    /// Check if a block is part of the current chain, that is, check if it is a direct ancestor of
    /// the current block.
    pub fn is_part_of_current_chain(&self, hash: &U256) -> Result<bool, Error> {
        let height = self.get_block_height(hash)?;  // height of block in question
        let member = self.get_current_block_of_height(height)?;  // member of current chain at that height
        Ok(*hash == member)  // part of chain iff it is the member at that height
    }



    /// Get the cached height of an existing block.
    pub fn get_block_height(&self, hash: &U256) -> Result<u64, Error> {
        if *hash == self.head.block { return Ok(self.head.height); }
        
        let key = Self::with_prefix(HEIGHT_BY_BLOCK_PREFIX, &hash.to_vec());
        let raw = self.get_raw_data(&key, CACHE_POSTFIX)?;

        Ok(bincode::deserialize::<u64>(&raw)?)
    }

    /// Find the height of a new block based on the height of its previous block.
    #[inline]
    pub fn get_new_block_height(&self, header: &BlockHeader) -> Result<u64, Error> {
        Ok(self.get_block_height(&header.prev)? + 1)
    }

    /// Get the key value for the height cache in the database. (Without the cache postfix).
    pub fn get_blocks_by_height_key(height: u64) -> Vec<u8> {
        let key: Vec<u8> = bincode::serialize(&height, bincode::Bounded(8)).unwrap();
        Self::with_prefix(BLOCKS_BY_HEIGHT_PREFIX, &key)
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

    /// Get blocks before the `target` hash until it collides with the main chain. If the `start`
    /// hash lies between the target and the main chain, it will return the blocks between them,
    /// otherwise it will return the blocks from the main chain until target in that order and it
    /// will not include the start or target blocks.
    ///
    /// If the limit is reached, it will prioritize blocks of a lower height, but may have a gap
    /// between the main chain (or start) and what it includes.
    pub fn get_blocks_before(&self, target: &U256, start: &U256, limit: usize)  -> Result<Vec<BlockHeader>, Error> {
        let mut iter = DownIter(&self, *target)
            .skip(1)
            .take(limit)
            .take_while(|res| res.is_ok())
            .map(|res| res.unwrap());
        
        // expand this as a loop to allow better error handling.
        let mut blocks: Vec<BlockHeader> = Vec::new();
        while let Some((hash, header)) = iter.next() {
            let in_cur_chain = self.is_part_of_current_chain(&hash)?;
            if (hash == *start) || (in_cur_chain) { break; }
            blocks.push(header)
        }
        
        blocks.reverse();
        Ok(blocks)
    }

    /// Retrieves all the blocks of the current chain which are a descendent of the latest common
    /// ancestor between the chain of the start block and the current chain. This result will be
    /// sorted in ascending height order. It will not include the start hash. Also, `limit` is the
    /// maximum number of blocks it should scan through when ascending the blockchain.
    pub fn get_blocks_after(&self, start: &U256, limit: usize) -> Result<Vec<BlockHeader>, Error> {
        // For efficiency, use a quick check to find if a given block is part of the current chain or not.
        let ancestor = self.latest_common_ancestor_with_current_chain(start)?;
        let ancestor_height = self.get_block_height(&ancestor)?;
        Ok(UpIter(&self, ancestor_height)
            .skip(1) // we know they have must have the LCA
            .take(limit) // hard-coded maximum
            .take_while(|res| res.is_ok()) // stop at first error, or when we reach the destination
            .map(|res| res.unwrap().1)  // exract block header
            .collect::<Vec<BlockHeader>>()
        )
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
            let result = self.get_blocks_of_height(height + 1);
            match result {
                Ok(blocks) => {
                    choice = *blocks.iter().nth(0).expect("Empty height in database!")
                },
                Err(e) => match e {
                    Error::NotFound(..) => return Ok(choice), // End loop when we reach a beyond what we know
                    e @ _ => return Err(e)
                }
            }
            height += 1;
        }
    }

    /// Walk the network state to a given block in the block chain
    pub fn walk(&mut self, b_block: &U256) -> Result<(), Error> {
        let a_block = self.head.block;
        assert!(!b_block.is_zero());
        
        { // verify that we are not crossing shards within reason
            let a_head = self.get_block_header(&a_block)?;
            let b_head = self.get_block_header(b_block)?;
            assert!(a_head.shard == b_head.shard || a_head.shard.is_zero() || b_head.shard.is_zero());
        }

        let (a_hashes, b_hashes, last_a, last_b) =
                self.latest_common_ancestor(&a_block, b_block)?;
        let (a_dist, b_dist) = Self::intersect_dist(&a_hashes, &b_hashes, &last_a, &last_b);

        let a_height = self.head.height;
        let b_height = a_height - a_dist + b_dist;
        
        // create lists of the `a` and `b` chains sorted by height
        let a_heights: BTreeMap<u64, U256> = a_hashes.into_iter()
            .filter(|&(_, d)| d <= a_dist)  // keep only values before intersection
            .map(|(k, d)| {
                assert!(a_height >= d);
                (a_height - d, k)
        }).collect();

        let b_heights: BTreeMap<u64, U256> = b_hashes.into_iter()
            .filter(|&(_, d)| d <= b_dist)  // keep only values before intersection
            .map(|(k, d)| {
                assert!(b_height >= d);
                (b_height - d, k)
        }).collect();


        { // verify validity; remove this check later on
            let mut collision = None;
            let mut last = None; // (height, block_hash)

            for (&h, &b) in &a_heights {
                if let Some((lh, _)) = last {
                    assert_eq!(h, lh - 1);
                } else { //check first element
                    collision = Some(b);
                    assert_eq!(h, a_height - a_dist);  // collision should be the first
                }
                last = Some((h, b));
            }{ // check last element
                let (h, b) = last.unwrap();
                assert_eq!(a_block, b);
                assert_eq!(a_height, h);
            }

            last = None;
            for (&h, &b) in &b_heights {
                if let Some((lh, _)) = last {
                    assert_eq!(h, lh - 1);
                } else { //check first element
                    assert_eq!(collision.unwrap(), b);
                    assert_eq!(h, a_height - a_dist);  // collision should be the first
                }
                last = Some((h, b));
            }{ // check last element
                let (h, b) = last.unwrap();
                assert_eq!(*b_block, b);
                assert_eq!(b_height, h);
            }
        }

        // go down `a` chain and then go up `b` chain.
        for (h, b) in a_heights.iter().rev() {
            assert!(*h > 0);
            let header = self.get_block_header(&b)?;
            let contra = self.get_contra(&b)?;
            self.undo_mutate(contra)?;
            self.head = HeadRef{block: header.prev, height: h - 1};
        }
        for (h, b) in b_heights {
            assert!(h > 1);
            let block = self.get_block(&b)?;
            let mutation = self.get_mutation(&block)?;
            let contra = self.mutate(&mutation)?;
            self.add_contra(&b, &contra)?;
            self.update_current_chain(h, &b)?;
            self.head = HeadRef{block: b, height: h};
        }

        debug!("Walked network state from {} to {}.", a_block, b_block);
        Ok(())
    }

    /// Find the current head of the block chain and then walk to it.
    #[inline]
    pub fn walk_to_head(&mut self) -> Result<(), Error> {
        let head = self.find_chain_head()?;
        self.walk(&head)
    }

    /// Add a new event to a plot
    pub fn add_plot_event(&mut self, plot_id: PlotID, tick: u64, event: &PlotEvent) -> Result<(), Error> {
        let db_key = Self::with_pre_post_fix(PLOT_PREFIX, &plot_id.bytes(), NETWORK_POSTFIX);

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
    pub fn get_plot_events(&self, plot_id: PlotID, _after_tick: u64) -> Result<PlotEvents, Error> {
        let db_key = Self::with_pre_post_fix(PLOT_PREFIX, &plot_id.bytes(), NETWORK_POSTFIX);

        Ok(self.db.get(&db_key)?.map_or(
            PlotEvents::new(),
            |v| bincode::deserialize(&v).unwrap()
        ))
    }

    /// Put together a mutation object from all of the individual transactions
    pub fn get_block_mutation(&self, block: &Block) -> Result<Mutation, Error> {
        let mut mutation = Mutation::new();
        for txn_hash in &block.txns {
            let txn = self.get_txn(&txn_hash)?;
            mutation.merge_clone(&txn.mutation);
        }
        Ok(mutation)
    }


    /// Mutate the stored **network state** and return a contra mutation to be able to undo what was
    /// done. Note that changes to either blockchain state or gamestate must occur through other
    /// functions.
    fn mutate(&mut self, mutation: &Mutation) -> Result<Mutation, Error> {
        mutation.assert_not_contra();
        let mut contra = Mutation::new_contra();

        for change in &mutation.changes { match change {
            &Change::SetValue{ref key, ref value, ..} => {
                let db_key = Self::with_postfix(&key, NETWORK_POSTFIX);
                
                contra.changes.push(Change::SetValue {
                    key: key.clone(),
                    value: self.db.get(&db_key)?.map(|v| v.to_vec()), // Option<Bin>
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
    fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), Error> {
        mutation.assert_contra();

        for change in mutation.changes { match change {
            Change::SetValue{key, value, ..} => {
                let db_key = Self::with_postfix(&key, NETWORK_POSTFIX);

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
                let db_key = Self::with_prefix(PLOT_PREFIX, &id.bytes());

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
            if self.is_part_of_current_chain(&h)? {
                return Ok(h);
            }
        }
        unreachable!()
    }

    /// Add a block the set of known blocks at a given height and the height of the block. That is,
    /// save the block by its height and its height by its hash.
    #[inline]
    fn cache_block(&mut self, height: u64, block: &U256) -> Result<(), Error> {
        self.add_block_to_height(height, &block)?;
        self.add_height_for_block(height, &block)
    }

    /// Add a block to the blocks by height cache. That way when trying to find all blocks of a
    /// given height, it will be listed.
    fn add_block_to_height(&mut self, height: u64, hash: &U256) -> Result<(), Error> {
        let key = Self::get_blocks_by_height_key(height);
        let res = self.get_raw_data(&key, CACHE_POSTFIX);
        
        let mut height_vals: Vec<U256> = {
            match res {
                Ok(raw) => bincode::deserialize(&raw)?,
                Err(Error::NotFound(..)) => Vec::new(),
                Err(e) => return Err(e)
            }
        };

        height_vals.push(*hash);
        let raw = bincode::serialize(&height_vals, bincode::Infinite)?;
        self.put_raw_data(&key, &raw, CACHE_POSTFIX)
    }

    /// Used when walking, this moves a given block the front of the list of blocks for the height
    /// which indicates that it is part of the current chain.
    fn update_current_chain(&mut self, height: u64, hash: &U256) -> Result<(), Error> {
        let key = Self::get_blocks_by_height_key(height);
        let mut height_values: Vec<U256> = {
            let raw = self.get_raw_data(&key, CACHE_POSTFIX)?;
            bincode::deserialize(&raw).unwrap()
        };

        if let Some(index) =
            height_values.iter()
            .position(|h| *h == *hash)
        { // we found the one we want, now swap it with the one in the front and re-save it
            height_values.swap(0, index);
            let raw = bincode::serialize(&height_values, bincode::Infinite).unwrap();
            self.put_raw_data(&key, &raw, CACHE_POSTFIX)
        }
        else { // It was not in the list
            Err(Error::NotFound(CACHE_POSTFIX, key))
        }
    }

    /// Cache the height of a block so it can be easily looked up later on.
    fn add_height_for_block(&mut self, height: u64, block: &U256) -> Result<(), Error> {
        let key = Self::with_prefix(HEIGHT_BY_BLOCK_PREFIX, &block.to_vec());
        let raw = bincode::serialize(&height, bincode::Bounded(8)).unwrap();
        self.put_raw_data(&key, &raw, CACHE_POSTFIX)
    }

    /// Construct a mutation given a block and its transactions by querying the DB for the txns and
    /// then merging their mutations.
    fn get_mutation(&self, block: &Block) -> Result<Mutation, Error> {
        let mut mutation = Mutation::new();
        for txn_h in &block.txns {
            let txn = self.get_txn(txn_h)?;
            mutation.merge(txn.mutation);
        }
        Ok(mutation)
    }

    /// Retrieve the contra from the db to undo the given block
    fn get_contra(&self, hash: &U256) -> Result<Mutation, Error> {
        let key = Self::with_prefix(CONTRA_PREFIX, &hash.to_vec());
        let raw = self.get_raw_data(&key, CACHE_POSTFIX)?;
        Ok(bincode::deserialize(&raw)?)
    }

    /// Add a contra for a given block
    fn add_contra(&mut self, hash: &U256, contra: &Mutation) -> Result<(), Error> {
        let key = Self::with_prefix(CONTRA_PREFIX, &hash.to_vec());
        let raw = bincode::serialize(contra, bincode::Infinite).unwrap();
        self.put_raw_data(&key, &raw, CACHE_POSTFIX)
    }


    /// Add a prefix to raw data.
    #[inline]
    pub fn with_prefix(prefix: &'static [u8], data: &[u8]) -> Vec<u8> {
        let mut t = Vec::from(prefix);
        t.extend_from_slice(data); t
    }

    /// Add a postfix to raw data
    #[inline]
    pub fn with_postfix(data: &[u8], postfix: &'static [u8]) -> Vec<u8> {
        let mut t = Vec::from(data);
        t.extend_from_slice(postfix); t
    }

    /// Add a prefix and postfix to raw data.
    #[inline]
    pub fn with_pre_post_fix(prefix: &'static [u8], data: &[u8], postfix: &'static [u8]) -> Vec<u8> {
        let mut t = Vec::from(prefix);
        t.extend_from_slice(data);
        t.extend_from_slice(postfix); t
    }

    /// Get the distance of the inrsection for the LCA on both paths. Returns
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
        if self.1 <= self.0.head.height {
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
