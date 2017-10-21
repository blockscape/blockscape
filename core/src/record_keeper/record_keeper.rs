use primitives::Mutation;
use std::path::PathBuf;
use std::sync::RwLock;
use super::{MutationRule, MutationRules, Error};
use super::database::Database;

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

    
}