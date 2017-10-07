use block::Block;
use env;
use mutation::{Change, Mutation};
use rocksdb::{DB, WriteBatch};
use rocksdb::Error as DBError;
use std::collections::LinkedList;
use std::fmt::Debug;
use std::sync::RwLock;
use txn::Txn;

/// Generic definition of a rule regarding whether changes to the database are valid.
/// Debug implementations should state what the rule means/requires.
trait MutationRule: Debug {
    /// Return Ok if it is valid, or an error explaining what rule was broken.
    fn is_valid(&self, database: &DB, mutation: &Mutation) -> Result<(), String>;
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
                k.extend_from_slice(NETWORK_POSTFIX);
                k
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
                k.extend_from_slice(NETWORK_POSTFIX);
                k
            };
            batch.put(&key, &change.value).expect("Failure when adding to rocksdb batch.");
        }

        (*db_lock).write(batch).map_err(|e| e.to_string())
    }

    // pub fn get_game_data(&self, mutation: )

    // pub fn get_block(hash: U256) -> Option<Block> {

    // }

    // pub fn get_txn(hash: U256) -> Option<Txn> {

    // }
}
