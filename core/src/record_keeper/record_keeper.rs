use bincode;
use primitives::Mutation;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::RwLock;
use super::{MutationRule, MutationRules, Error, Storable};
use super::database::*;
use primitives::U256;

/// An abstraction on the concept of states and state state data. Builds higher-level functunality
/// On top of the database. The implementation uses RwLocks to provide many read, single write
/// thread safety.
pub struct RecordKeeper {
    db: RwLock<Database>,
    rules: RwLock<MutationRules>,
}

impl RecordKeeper {
    /// Construct a new RecordKeeper from an already opened database and possibly an existing set of
    /// rules.
    pub fn new(db: Database, rules: Option<MutationRules>) -> RecordKeeper {
        RecordKeeper{
            db: RwLock::new(db),
            rules: RwLock::new(rules.unwrap_or(MutationRules::new())),
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

    /// Retrieve and deserialize data from the database. This will return an error if the database
    /// has an issue, if the data cannot be deserialized or if the object is not present in the
    /// database. Note that `instance_id` should be the object's ID/key which would normally be
    /// returned from calling `storable.instance_id()`.
    fn get<S: Storable>(&self, instance_id: &[u8], postfix: &'static [u8]) -> Result<S, Error> {
        let key = {
            let mut k = Vec::from(S::global_id());
            k.extend_from_slice(instance_id); k
        };

        let raw = self.get_raw_data(&key, postfix)?;
        bincode::deserialize::<S>(&raw)
        .map_err(|e| Error::Deserialize(e.to_string()))
    }

    /// Serialize and store data in the database. This will return an error if the database has an
    /// issue.
    fn put<S: Storable>(&mut self, obj: &S, postfix: &'static [u8]) -> Result<(), Error> {
        let value = bincode::serialize(obj, bincode::Infinite)
            .expect("Error serializing game data.");
        self.put_raw_data(&obj.key(), &value, postfix)
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
        self.get::<S>(instance_id, CACHE_POSTFIX)
    }

    /// Put cache data into the database. This is for library use only.
    pub fn put_cache_data<S: Storable>(&mut self, obj: &S) -> Result<(), Error> {
        self.put::<S>(obj, CACHE_POSTFIX)
    }

    /// Retrieve blockchain data from the database. Use this for things like Blocks or Txns.
    pub fn get_blockchain_data<S: Storable>(&self, hash: &U256) -> Result<S, Error> {
        let mut id: [u8; 32] = [0u8; 32];
        hash.to_little_endian(&mut id);
        self.get::<S>(&id, BLOCKCHAIN_POSTFIX)
    }
}