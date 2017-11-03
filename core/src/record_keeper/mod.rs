pub mod database;
pub mod error;
pub mod storable;
pub mod plot_event;

mod mutation_rule;
mod record_keeper;

pub use self::error::Error;
pub use self::mutation_rule::{MutationRule, MutationRules};
pub use self::record_keeper::{RecordKeeper, RecordEvent};
pub use self::storable::Storable;
pub use self::plot_event::PlotEvent;


use primitives;
/// A unique plot identification marker based on it's (x,y) coordinate.
pub type PlotID = primitives::Cord;