use primitives::{Mutation, U256, Event};
use std::collections::LinkedList;
use std::fmt::Debug;
use super::database::Database;

/// Generic definition of a rule regarding whether changes to the database are valid.
/// Debug implementations should state what the rule means/requires.
pub trait MutationRule<E: Event>: Debug + Send + Sync {
    /// Return Ok if it is valid, or an error explaining what rule was broken.
    fn is_valid(&self, database: &Database, mutation: &Mutation<E>) -> Result<(), String>;
}

/// A list of mutation rules
pub type MutationRules<E: Event> = LinkedList<Box<MutationRule<E>>>;


/// Thrown by a mutation if it is invalid
pub enum InvalidMutation {

}