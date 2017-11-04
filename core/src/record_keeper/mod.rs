pub mod database;
pub mod error;
pub mod events;
pub mod storable;

mod mutation_rule;
mod record_keeper;

pub use self::error::Error;
pub use self::events::{PlotEvent, RecordEvent};
pub use self::mutation_rule::{MutationRule, MutationRules};
pub use self::record_keeper::{RecordKeeper};
pub use self::storable::Storable;


use primitives;
/// A unique plot identification marker based on it's (x,y) coordinate.
pub type PlotID = primitives::Cord;