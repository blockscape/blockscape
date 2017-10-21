use primitives::Mutation;
use std::collections::LinkedList;
use std::fmt::Debug;
use super::database::Database;

/// Generic definition of a rule regarding whether changes to the database are valid.
/// Debug implementations should state what the rule means/requires.
pub trait MutationRule: Debug + Send + Sync {
    /// Return Ok if it is valid, or an error explaining what rule was broken.
    fn is_valid(&self, database: &Database, mutation: &Mutation) -> Result<(), String>;
}

/// A list of mutation rules
pub type MutationRules = LinkedList<Box<MutationRule>>;