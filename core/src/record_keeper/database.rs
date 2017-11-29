use bincode;
use bytes::{BigEndian, ByteOrder};
use env;
use primitives::{U256, Mutation, Change, Block, BlockHeader, Txn};
use rocksdb::{DB, Options};
use rocksdb::Error as RocksDBError;
use std::collections::{HashSet, HashMap, BTreeMap};
use std::path::PathBuf;
use super::{Storable, PlotEvent, PlotEvents, events, PlotID};
use super::error::*;

pub const BLOCKCHAIN_POSTFIX: &[u8] = b"b";
pub const CACHE_POSTFIX: &[u8] = b"c";
pub const NETWORK_POSTFIX: &[u8] = b"n";

pub const PLOT_PREFIX: &[u8] = b"PLT";
pub const HEIGHT_PREFIX: &[u8] = b"HGT";
pub const CONTRA_PREFIX: &[u8] = b"CMT";

pub const CURRENT_BLOCK: &[u8] = b"CURblock";

#[inline]
fn extend_vec(mut k: Vec<u8>, post: &[u8]) -> Vec<u8> {
    k.extend_from_slice(post); k
}


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

impl HeadRef {
    #[inline]
    pub fn is_null(&self) -> bool { self.block.is_zero() }
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
    pub fn get_raw_data(&self, key: &[u8], postfix: &'static [u8]) -> Result<Vec<u8>, Error> {
        Self::get_raw_data_static(&self.db, key, postfix)
    }

    fn get_raw_data_static(db: &DB, key: &[u8], postfix: &'static [u8]) -> Result<Vec<u8>, Error> {
        let key = {
            let mut k = Vec::from(key);
            k.extend_from_slice(postfix); k
        };

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
        let key = {
            let mut k = Vec::from(key);
            k.extend_from_slice(postfix); k
        };

        Ok(db.put(&key, &data)?)
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
        let raw_txns = bincode::serialize(&block.transactions, bincode::Infinite)
                .expect("Error serilizing transactions!");
        self.put_raw_data(&merkle_root, &raw_txns, BLOCKCHAIN_POSTFIX)?;
        
        // add the block to the height cache
        let hash = block.calculate_hash();
        let height = self.get_block_height(&hash)?;
        self.add_block_to_height(height, hash)?;
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

    /// Retrieve the transactions for a block to complete a `BlockHeader` as a `Block` object.
    pub fn complete_block(&self, header: BlockHeader) -> Result<Block, Error> {
        let merkle_root = header.merkle_root.to_vec();
        let raw_txns = self.get_raw_data(&merkle_root, BLOCKCHAIN_POSTFIX)?;
        Ok(Block::deserialize(header, &raw_txns)?)
    }

    pub fn get_txn(&self, hash: &U256) -> Result<Txn, Error> {
        let id = hash.to_vec();
        self.get::<Txn>(&id, BLOCKCHAIN_POSTFIX)
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

    /// Determine the height of a given block. It will follow the path until it finds the genesis
    /// block which is denoted by having a previous block reference of 0.
    pub fn get_block_height(&self, hash: &U256) -> Result<u64, Error> {
        // Height of the null block before genesis is zero.
        if hash.is_zero() { return Ok(0); }

        let (a, b, last_a, last_b) = self.latest_common_ancestor(&self.head.block, &hash)?;

        if last_b.is_zero() {
            // b reached the genesis block before intersecting, so we know the height is equal to
            // its distance.
            return Ok(*b.get(&last_b).unwrap())
        }

        // We have had a collision! Now we can calculate the height.
        let (a_dist, b_dist) = Self::intersect_dist(&a, &b, &last_a, &last_b);
        assert!(self.head.height >= a_dist);
        let intersect_height = self.head.height - a_dist; // the height of the last common ancestor
        
        // total distance is height from LCA to the distance up another chain it must go
        Ok(intersect_height + b_dist)
    }

    /// Get the key value for the height cache in the database. (Without the cache postfix).
    pub fn get_block_height_key(height: u64) -> Vec<u8> {
        let mut buf = [0u8; 8];
        BigEndian::write_u64(&mut buf, height);
        let mut k = Vec::from(HEIGHT_PREFIX);
        k.extend_from_slice(&buf); k
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

    /// Put together a mutation object from all of the individual transactions
    pub fn get_block_mutation(&self, block: &Block) -> Result<Mutation, Error> {
        let mut mutation = Mutation::new();
        for txn_hash in &block.transactions {
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
    fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), Error> {
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
        
        // insert the starting blocks with a distnce from themselves of zero
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

    /// Add a block the set of known blocks at a given height.
    fn add_block_to_height(&mut self, height: u64, block: U256) -> Result<(), Error> {
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

    /// Construct a mutation given a block and its transactions by querying the DB for the txns and
    /// then merging their mutations.
    fn get_mutation(&self, block: &Block) -> Result<Mutation, Error> {
        let mut mutation = Mutation::new();
        for txn_h in &block.transactions {
            let txn = self.get_txn(txn_h)?;
            mutation.merge(txn.mutation);
        }
        Ok(mutation)
    }

    /// Retrieve the contra from the db to undo the given block
    fn get_contra(&self, hash: &U256) -> Result<Mutation, Error> {
        let key = Self::contra_kay(hash);
        let raw = self.get_raw_data(&key, CACHE_POSTFIX)?;
        Ok(bincode::deserialize(&raw)?)
    }

    /// Add a contra for a given block
    fn add_contra(&mut self, hash: &U256, contra: &Mutation) -> Result<(), Error> {
        let key = Self::contra_kay(hash);
        let raw = bincode::serialize(contra, bincode::Infinite).unwrap();
        self.put_raw_data(&key, &raw, CACHE_POSTFIX)
    }


    fn plot_key(id: &PlotID) -> Vec<u8> {
        let mut k = Vec::from(PLOT_PREFIX);
        k.append(&mut id.bytes());
        k.extend_from_slice(NETWORK_POSTFIX); k
    }

    fn contra_kay(hash: &U256) -> Vec<u8> {
        let mut k = Vec::from(CONTRA_PREFIX);
        k.append(&mut hash.to_vec()); k        
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