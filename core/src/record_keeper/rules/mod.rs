/// All defined block rules
pub mod block;
/// All defined txn rules
pub mod txn;
/// Some pre-defined mutation rules independent of game logic
pub mod mutation;


use primitives::{Change, Block, Txn, U160, Event};
use std::collections::LinkedList;
use std::fmt::Debug;
use super::database::Database;
use super::{Error, NetState, PlotEvent};
use bin::Bin;
use std::fmt;
use serde::de::DeserializeOwned;
use bincode;


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


/// Generic definition of a rule regarding whether changes to the database are valid. Debug
/// implementations should state what the rule means/requires.
/// #TODO
/// We will need to take in a GameState object of some sort as well, and to be able to request one
/// at a certain point in the blockchain's history. This object should be of the game writer's
/// choosing and so will need to be templated or something. Ultimately the game state will be stored
/// in RAM, and they will need to keep multiple checkpoints to allow going backwards.
pub trait MutationRule: Send + Sync {
    /// Return Ok if it is valid, or an error explaining what rule was broken or what error was encountered.
    fn is_valid(&self, net_state: &NetState, mutation: &Vec<(Change, U160)>, cache: &mut Bin) -> Result<(), Error>;
    /// Retrieve a description of the rule.
    fn description(&self) -> &'static str;
}

/// Simplify iterating over PlotEvents for Mutation Rules.
pub fn plot_events_rule_iter<F>(mut func: F, mutation: &Vec<(Change, U160)>) -> Result<(), Error>
    where F: FnMut(&PlotEvent, U160) -> Result<(), Error>
{
    for &(ref change, ref user) in mutation {
        if let &Change::PlotEvent(ref pe) = change {
            func(pe, *user)?;
        }
    } Ok(())
}

/// Simply iterating over game events for Mutation Rules.
pub fn game_events_rule_iter<F, E>(mut func: F, mutation: &Vec<(Change, U160)>) -> Result<(), Error>
    where F: FnMut(E, U160) -> Result<(), Error>,
          E: Event + DeserializeOwned
{
    for &(ref change, ref user) in mutation {
        if let &Change::PlotEvent(ref pe) = change {
            func(bincode::deserialize(&pe.event)?, *user)?;
        }
    } Ok(())
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