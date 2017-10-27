use bincode;
use bytes::{BigEndian, ByteOrder};
use primitives::{U256, Txn, Block, BlockHeader, Mutation};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::RwLock;
use super::{MutationRule, MutationRules, Error, Storable};
use super::database::*;

const HEIGHT_PREFIX: &[u8] = b"h";

/// An abstraction on the concept of states and state state data. Builds higher-level functunality
/// On top of the database. The implementation uses RwLocks to provide many read, single write
/// thread safety.
/// TODO: Also add a block to the known blocks if it is only referenced.
pub struct RecordKeeper {
    db: RwLock<Database>,
    rules: RwLock<MutationRules>,
    pending_txns: RwLock<HashMap<U256, Txn>>
}

impl RecordKeeper {
    /// Construct a new RecordKeeper from an already opened database and possibly an existing set of
    /// rules.
    pub fn new(db: Database, rules: Option<MutationRules>) -> RecordKeeper {
        RecordKeeper{
            db: RwLock::new(db),
            rules: RwLock::new(rules.unwrap_or(MutationRules::new())),
            pending_txns: RwLock::new(HashMap::new())
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

    pub fn add_block(&mut self, block: &Block) -> Result<(), Error> {
        let block_hash = block.header.calculate_hash();
        let block_height = self.get_block_height(&block.header.prev)?;
        self.add_block_to_height(block_height, block_hash)?;

        unimplemented!("Need to add a block and move the state forward if it is longer than the current chain")
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
        mutation.assert_not_contra();
        let mut db_lock = self.db.write().unwrap();
        self.is_valid_given_lock(&*db_lock, mutation).map_err(|e| Error::InvalidMut(e))?;
        
        db_lock.mutate(mutation)
    }

    /// Apply a contra mutation to the network state. (And consumes the mutation).
    fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), Error> {
        mutation.assert_contra();
        let mut db_lock = self.db.write().unwrap();
        
        db_lock.undo_mutate(mutation)
    }

    /// Internal use function to check if a mutation is valid given a lock of the db. While it only
    /// takes a reference to a db, make sure a lock is in scope which is used to get the db ref.
    fn is_valid_given_lock(&self, db: &Database, mutation: &Mutation) -> Result<(), String> {
        let rules_lock = self.rules.read().unwrap();
        for rule in &*rules_lock {
            // verify all rules are satisfied and return, propagate error if not
            rule.is_valid(db, mutation)?;
        }
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