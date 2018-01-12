/// All defined block rules
pub mod block;
/// All defined txn rules
pub mod txn;


use primitives::{Mutation, Block, Txn};
use std::collections::LinkedList;
use std::fmt::Debug;
use super::database::Database;
use super::{Error, NetState};
use bin::Bin;
use std::fmt;


/// A rule which is responsible for assessing if the high-level block structure is valid.
pub trait BlockRule: Send + Sync {
    /// Return Ok if it is valid, or an error explaining what rule was broken or what error was
    /// encountered.
    /// **Note: There is overlap between the information in DB and NetState, use DB only for
    /// Chainstate and Cachestate, do not use it for the Networkstate.**
    fn is_valid(&self, state: &NetState, db: &Database, block: &Block) -> Result<(), Error>;
    /// Retrieve a description of the rule.
    fn description(&self) -> &'static str;
}


/// A rule which is responsible for assessing if an individual transaction is valid. Mostly from a
/// clerical standpoint as it does not consider all txns together.
pub trait TxnRule: Send + Sync {
    /// Return Ok if it is valid, or an error explaining what rule was broken or what error was encountered.
    fn is_valid(&self, state: &NetState, txn: &Txn) -> Result<(), Error>;
    /// Retrieve a description of the rule.
    fn description(&self) -> &'static str;
}


/// Generic definition of a rule regarding whether changes to the database are valid.
/// Debug implementations should state what the rule means/requires.
pub trait MutationRule: Send + Sync {
    /// Return Ok if it is valid, or an error explaining what rule was broken or what error was encountered.
    fn is_valid(&self, net_state: &NetState, mutation: &Mutation, cache: &mut Bin) -> Result<(), Error>;
    /// Retrieve a description of the rule.
    fn description(&self) -> &'static str;
}

/// A list of mutation rules
pub type MutationRules = LinkedList<Box<MutationRule>>;



// Wrap all of the rules with Debug trait which simply calls their description function.
impl Debug for BlockRule {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(self.description())
    }
}

impl Debug for TxnRule {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(self.description())
    }
}

impl Debug for MutationRule {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(self.description())
    }
}