use bincode;
use env;
use mutation::{Change, Mutation};
use rocksdb::{DB, WriteBatch};
use rocksdb::Error as RocksDBError;
use serde;
use std::collections::LinkedList;
use std::error::Error as StdErr;
use std::fmt;
use std::fmt::{Debug, Display};
use std::sync::RwLock;
use u256::U256;


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
pub trait Storable: serde::Serialize + serde::de::DeserializeOwned {
    /// Error to be returned if it could not be deserialized correctly.
    // type DeserializeErr;

    /// Return a unique ID for the type, an example of this is b"plot", though the smallest
    /// reasonable values would be better, e.g. b"p" for plot. All storable types must return
    /// different IDs or there may be collisions.
    fn global_id() -> &'static [u8];

    /// Calculate and return a unique ID for the instance of this storable value. In the case of a
    /// plot, it would simply be the plot ID. It is for a block, then it would just be its Hash.
    /// This must not change between saves and loads for it to work correctly.
    fn instance_id(&self) -> Vec<u8>;

    /// Calculate and return the key-value of this object based on its global and instance IDs.
    fn key(&self) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(Self::global_id());
        key.append(&mut self.instance_id());
        key
    }
}


#[derive(Debug)]
pub enum Error {
    DB(RocksDBError), // when there is an error working with the database itself
    NotFound(&'static [u8], &'static [u8], Vec<u8>), // when data is not found in the database
    Deserialize(String), // when data cannot be deserialized
    InvalidMut(String) // when a rule is broken by a mutation
}

impl StdErr for Error {
    fn description(&self) -> &str {
        match *self { //TODO: why can we just get a ref of the objects
            Error::DB(_) => "RocksDB error: aka, not my fault ☺",
            Error::NotFound(_, _, _) => "Could not find the data requested at that Hash (may not be an issue).",
            Error::Deserialize(_) => "Deserialization error, the data stored could not be deserialized into the requested type.",
            Error::InvalidMut(_) => "Invalid Mutation, a rule is violated by the mutation so it will not be applied."
        }
    }

    fn cause(&self) -> Option<&StdErr> {
        match *self {
            Error::DB(ref e) => Some(e),
            Error::NotFound(_, _, _) => None,
            Error::Deserialize(_) => None,
            Error::InvalidMut(_) => None,
        }
    }
}

impl From<RocksDBError> for Error {
    fn from(e: RocksDBError) -> Self { Error::DB(e) }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.description())
    }
}


/// A list of mutation rules
pub type MutationRules = LinkedList<Box<MutationRule>>;


const BLOCKCHAIN_POSTFIX: &[u8] = b"b";
const CACHE_POSTFIX: &[u8] = b"c";
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
    pub fn open_db(rules: Option<MutationRules>) -> Result<Database, RocksDBError> {
        let mut dir = env::get_storage_dir().unwrap();
        dir.push("db");

        Ok(
            DB::open_default(dir)
            .map(|db| Self::new(db, rules))?
        )
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
    fn is_valid_given_lock(&self, db: &DB, mutation: &Mutation) -> Result<(), String> {
        let rules_lock = self.rules.read().unwrap();
        for rule in &*rules_lock {
            // verify all rules are satisfied and return, propagate error if not
            rule.is_valid(db, mutation)?;
        }
        Ok(())
    }

    /// Mutate the stored **network state** and return a contra mutation to be able to undo what was
    /// done. Note that changes to either blockchain state or gamestate must occur through other
    /// functions.
    pub fn mutate(&mut self, mutation: &Mutation) -> Result<Mutation, Error> {
        mutation.assert_not_contra();
        let db_lock = self.db.write().unwrap();

        self.is_valid_given_lock(&*db_lock, mutation).map_err(|e| Error::InvalidMut(e))?;

        let mut contra = Mutation::new_contra();
        let mut batch = WriteBatch::default();
        for change in &mutation.changes {
            let key = {
                let mut k = change.key.clone();
                k.extend_from_slice(NETWORK_POSTFIX); k
            };
            
            // Result<Option<DBVector>, DBError>
            let prior_value = db_lock.get(&key)?.map(|v| v.to_vec());
            
            contra.changes.push(Change {
                key: key.clone(),
                value: prior_value,
                data: None,
            });

            if let Some(ref v) = change.value {
                batch.put(&key, v).expect("Failure when adding to rocksdb batch.");
            } else {  // delete key
                batch.delete(&key);
            }
        }
        (*db_lock).write(batch)?;

        contra.changes.reverse();
        Ok(contra)
    }

    /// Consumes a contra mutation to undo changes made by the corresponding mutation to the
    /// network state.
    pub fn undo_mutate(&mut self, mutation: Mutation) -> Result<(), RocksDBError> {
        mutation.assert_contra();
        let mut batch = WriteBatch::default();
        let db_lock = self.db.read().unwrap();
        for change in &mutation.changes {
            let key = {
                let mut k = change.key.clone();
                k.extend_from_slice(NETWORK_POSTFIX); k
            };

            if let Some(ref v) = change.value {
                batch.put(&key, v).expect("Failure when adding to rocksdb batch.");
            } else {  // delete key
                batch.delete(&key);
            }
        }

        (*db_lock).write(batch)
    }

    /// Retrieve network data from the database. Use this for things which are stored and modified
    /// by transactions like the list of validators and public keys.
    pub fn get_network_data(&self, key: &[u8]) -> Result<Vec<u8>, Error> {
        let key = {
            let mut k = Vec::from(key);
            k.extend_from_slice(key);
            k.extend_from_slice(NETWORK_POSTFIX); k
        };

        let db_lock = self.db.read().unwrap();

        db_lock.get(&key)?
            .map(|d| d.to_vec())
            .ok_or(Error::NotFound(NETWORK_POSTFIX, b"", Vec::from(key)))
    }

    /// Retrieve and deserialize data from the database. This will return an error if the database
    /// has an issue, if the data cannot be deserialized or if the object is not present in the
    /// database. Note that `instance_id` should be the object's ID/key which would normally be
    /// returned from calling `storable.instance_id()`.
    fn get<S: Storable>(&self, instance_id: &[u8], postfix: &'static [u8]) -> Result<S, Error> {
        let key = {
            let mut k = Vec::new();
            k.extend_from_slice(S::global_id());
            k.extend_from_slice(instance_id);
            k.extend_from_slice(postfix); k
        };

        let db_lock = self.db.read().unwrap();

        match db_lock.get(&key)? {
            Some(data) =>
                bincode::deserialize::<S>(&data)
                .map_err(|e| Error::Deserialize(e.to_string())),
            None => Err(Error::NotFound(postfix, S::global_id(), Vec::from(instance_id)))
        }
    }

    /// Serialize and store data in the database. This will return an error if the database has
    /// an issue.
    fn put<S: Storable>(&mut self, obj: &S, postfix: &[u8]) -> Result<(), Error> {
        let key = {
            let mut k = obj.key();
            k.extend_from_slice(postfix); k
        };

        let value = bincode::serialize(obj, bincode::Infinite).expect("Error serializing game data.");
        
        let db_lock = self.db.write().unwrap();
        Ok(db_lock.put(&key, &value)?)
    }

    /// Retrieve blockchain data from the database. Use this for things like Blocks or Txns.
    pub fn get_blockchain_data<S: Storable>(&self, hash: &U256) -> Result<S, Error> {
        let mut id: [u8; 32] = [0u8; 32];
        hash.to_little_endian(&mut id);

        self.get::<S>(&id, BLOCKCHAIN_POSTFIX)
    }


    /// Write a blockchain object into the database. Use this for things like Blocks or Txns. With generics:
    pub fn put_blockchain_data<S: Storable>(&mut self, obj: &S) -> Result<(), Error> {
        self.put::<S>(obj, BLOCKCHAIN_POSTFIX)
    }

    /// Retrieve cache data from the database. This is for library use only.
    pub fn get_cache_data<S: Storable>(&self, instance_id: &[u8]) -> Result<S, Error> {
        self.get::<S>(instance_id, CACHE_POSTFIX)
    }

    /// Put cache data into the database. This is for library use only.
    pub fn put_cache_data<S: Storable>(&mut self, obj: &S) -> Result<(), Error> {
        self.put::<S>(obj, CACHE_POSTFIX)
    }
}