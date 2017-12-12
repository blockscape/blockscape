pub mod block_package;
pub mod database;
pub mod error;
pub mod events;
pub mod storable;

mod rules;
mod record_keeper;

pub use self::block_package::BlockPackage;
pub use self::error::{Error, LogicError};
pub use self::events::{PlotEvent, PlotEvents, RecordEvent};
pub use self::record_keeper::{RecordKeeper};
pub use self::rules::*;
pub use self::storable::Storable;


use primitives;
/// A unique plot identification marker based on it's (x,y) coordinate.
pub type PlotID = primitives::Cord;