use block::Block;
use env;
use mutation::{Change, Mutation};
use rocksdb::{DB, WriteBatch};
use rocksdb::Error as DBError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::LinkedList;
use std::fmt::Debug;
use std::sync::RwLock;
use txn::Txn;
use bincode;
use u256::U256;
use hash::hash_obj;

/// Generic definition of a rule regarding whether changes to the database are valid.
/// Debug implementations should state what the rule means/requires.
pub trait MutationRule: Debug + Send + Sync {
    /// Return Ok if it is valid, or an error explaining what rule was broken.
    fn is_valid(&self, database: &DB, mutation: &Mutation) -> Result<(), String>;
}

/// Storable objects are able to be stored in a `Database` instance.
/// Example implementation:
/// ```
/// #[derive(Serialize, Deserialize)]
/// struct Example {
///     a: u8,
///     b: u8
/// }
///
/// impl Storable for Example {
///     type DeserializeErr = String;
///     fn global_id() -> &'static [u8] { b"p" }
///     fn instance_id(&self) -> Vec<u8> { vec![self.a, self.b] }
/// }
/// ```
pub trait Storable: Serialize + DeserializeOwned {
    /// Error to be returned if it could not be deserialized correctly.
    type DeserializeErr;

    /// Return a unique ID for the type, an example of this is b"plot", though the smallest
    /// reasonable values would be better, e.g. b"p" for plot. All storable types must return
    /// different IDs or there may be collisions.
    fn global_id() -> &'static [u8];

    /// Calculate and return a unique ID for the instance of this storable value. In the case of a
    /// plot, it would simply be the plot ID. Must be
    fn instance_id(&self) -> Vec<u8>;

    /// Calculate and return the key-value of this object based on its global and instance IDs.
    fn key(&self) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(Self::global_id());
        key.append(&mut self.instance_id());
        key
    }
}


/// A list of mutation rules
pub type MutationRules = LinkedList<Box<MutationRule>>;


const BLOCKCHAIN_POSTFIX: &[u8] = b"b";
const GAME_POSTFIX: &[u8] = b"g";
const NETWORK_POSTFIX: &[u8] = b"n";

/// This is a wrapper around a RocksDB instance to provide the access and modifications needed for
/// our system. The implementation uses RwLocks to provide many read, single write thread safety.
/// Please note that there are three distinct "regions" of the database:
/// - The **blockcahin state** stores the blocks and transactions by their hashes.
/// - The **game state** stores plots and their associated data, possibly other things as well.
/// - The **network state** stores the results of transactions being applied, things like who is a
///   valid miner, reputations, checkpoints/snapshots, and more.
/// To keep these regions separate, postfixes are appended before accessing the database, this will
/// prevent conflicts between the different regions even if they are using non-secure hashing
/// methods.
pub struct Database {
    db: RwLock<DB>,
    rules: RwLock<MutationRules>,
}


impl Database {
    /// Create a new Database from a RocksDB instance
    pub fn new(db: DB, rules: Option<MutationRules>) -> Database {
        Database {
            db: RwLock::new(db),
            rules: RwLock::new(rules.unwrap_or(MutationRules::new())),
        }
    }

    /// Open the RocksDB database based on the environment
    pub fn open_db(rules: Option<MutationRules>) -> Result<Database, DBError> {
        let mut dir = env::get_storage_dir().unwrap();
        dir.push("db");

        DB::open_default(dir).map(|db| Self::new(db, rules))
    }

    /// Add a new rule to the database regarding what network mutations are valid. This will only
    /// impact things which are mutated through the `mutate` function.
    pub fn add_rule(&mut self, rule: Box<MutationRule>) {
        let mut rules_lock = self.rules.write().unwrap();
        (*rules_lock).push_back(rule);
    }

    /// Check if a mutation to the network state is valid.
    pub fn is_valid(&self, mutation: &Mutation) -> Result<(), String> {
        let db_lock = self.db.read().unwrap();
        self.is_valid_given_lock(&*db_lock, mutation)
    }

    /// Internal use function to check if a mutation is valid given a lock of the db. While it only
    /// takes a reference to a db, make sure a lock is in scope which is used to get the db ref.
    fn is_valid_given_lock(&self, db: &DB, mutation: &Mutation) -> Result<(), String> {
        let rules_lock = self.rules.read().unwrap();
        for rule in &*rules_lock {
            // verify all rules are satisfied and return propagate error if not
            rule.is_valid(db, mutation)?;
        }
        Ok(())
    }

    /// Mutate the stored network state and return a contra mutation to be able to undo what was
    /// done. Note that changes to either blockchain state or gamestate must occur through other
    /// functions.
    pub fn mutate(&mut self, mutation: &Mutation) -> Result<Mutation, String> {
        mutation.assert_not_contra();
        let db_lock = self.db.write().unwrap();

        self.is_valid_given_lock(&*db_lock, mutation)?;

        let mut contra = Mutation::new_contra();
        let mut batch = WriteBatch::default();
        for change in &mutation.changes {
            let key = {
                let mut k = change.key.clone();
                k.extend_from_slice(NETWORK_POSTFIX); k
            };
            
            let prior_value = db_lock.get(&key) // Result<Option<DBVector>, DBError>
                .map_err(|e| e.to_string())?
                .map_or(Vec::new(), |v| v.to_vec());
            
            contra.changes.push(Change {
                key: key.clone(),
                value: prior_value,
                data: None,
            });

            batch.put(&key, &change.value).expect("Failure when adding to rocksdb batch.");
        }
        (*db_lock).write(batch).map_err(|e| e.to_string())?;

        contra.changes.reverse();
        Ok(contra)
    }

    /// Consumes a contra mutation to undo changes made by the corresponding mutation.
    pub fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), String> {
        mutation.assert_contra();
        let mut batch = WriteBatch::default();
        let db_lock = self.db.read().unwrap();
        for change in &mutation.changes {
            let key = {
                let mut k = change.key.clone();
                k.extend_from_slice(NETWORK_POSTFIX); k
            };
            batch.put(&key, &change.value).expect("Failure when adding to rocksdb batch.");
        }

        (*db_lock).write(batch).map_err(|e| e.to_string())
    }

    /// Retrieve and deserialize data from the database. This will return an error if either the
    /// database has an issue, or if the data cannot be deserialized. If the object is not present
    /// in the database, then None will be returned. Note that `instance_id` should be the object's
    /// ID/key which would normally be returned from calling `storable.instance_id()`.
    pub fn get<S: Storable>(&self, instance_id: &[u8]) -> Result<Option<S>, String>
    {
        let key = {
            let mut k = Vec::new();
            k.extend_from_slice(S::global_id());
            k.extend_from_slice(instance_id);
            k.extend_from_slice(GAME_POSTFIX); k
        };

        let db_lock = self.db.read().unwrap();
        let db_res = (*db_lock).get(&key).map_err(|e| e.to_string())?;

        match db_res {
            Some(data) => Ok(Some(
                bincode::deserialize::<S>(&data)
                .map_err(|e| e.to_string())?
            )),
            None => Ok(None),
        }
    }

    /// Serialize and store game data in the database.
    pub fn put<S: Storable>(&mut self, obj: &S) -> Result<(), String> {
        let key = {
            let mut k = obj.key();
            k.extend_from_slice(GAME_POSTFIX); k
        };

        let value = bincode::serialize(obj, bincode::Infinite).expect("Error serializing game data.");
        
        let db_lock = self.db.write().unwrap();
        (*db_lock).put(&key, &value).map_err(|e| e.to_string())
    }

    /// Retrieve a blockchain data from the database. Will return none if the data is not found, and
    /// DBError if something goes wrong when attempting to retrieve the data. It also assumes that
    /// hashes will not collide.
    /// # Panics
    /// This assumes it will be able to deserialize the data should it find the hash.
    pub fn get_blockchain_data<B>(&self, hash: &U256) -> Result<Option<B>, DBError>
        where B: Serialize + DeserializeOwned
    {
        let key = {
            let mut k = bincode::serialize(hash, bincode::Bounded(32)).unwrap();
            k.extend_from_slice(BLOCKCHAIN_POSTFIX); k
        };

        let db_lock = self.db.read().unwrap();
        let opt = (*db_lock).get(&key)?;
        
        Ok(
            opt.map(|data|
                bincode::deserialize::<B>(&data)
                .expect("Failure to deserialize block.")
            )
        )
    }

    /// Write a blockchain object into the database using its hash. Will return an error if the
    /// database has troubles. It also assumes that hashes will not collide.
    pub fn put_blockchain_data<B>(&mut self, obj: &B) -> Result<(), DBError>
        where B: Serialize + DeserializeOwned
    {
        let key = {
            let mut k = bincode::serialize(&hash_obj(obj), bincode::Bounded(32)).unwrap();
            k.extend_from_slice(BLOCKCHAIN_POSTFIX); k
        };
        let value = bincode::serialize(obj, bincode::Infinite)
            .expect("Error serializing blochain data");

        let db_lock = self.db.write().unwrap();
        (*db_lock).put(&key, &value)?;
        
        Ok(())
    }
}