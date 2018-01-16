pub mod block_package;
pub mod database;
pub mod error;
pub mod events;
pub mod net_dif;
pub mod net_state;
pub mod storable;

mod rules;
mod record_keeper;

pub use self::block_package::BlockPackage;
pub use self::error::{Error, LogicError};
pub use self::events::{PlotEvent, JPlotEvent, PlotEvents, RecordEvent};
pub use self::net_dif::*;
pub use self::net_state::*;
pub use self::record_keeper::{RecordKeeper};
pub use self::rules::*;
pub use self::storable::Storable;


use primitives;
/// A unique plot identification marker based on it's (x,y) coordinate.
pub type PlotID = primitives::Cord;