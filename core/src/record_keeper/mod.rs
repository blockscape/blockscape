pub mod block_package;
pub mod database;
pub mod error;
pub mod events;
pub mod key;
pub mod db_state;
pub mod game_state;

mod record_keeper;
mod dummy;
mod rules;

pub use self::block_package::BlockPackage;
pub use self::error::*;
pub use self::events::{PlotEvent, JPlotEvent, RecordEvent, DePlotEvent};
pub use self::db_state::*;
pub use self::record_keeper::*;
pub use self::dummy::*;
pub use self::rules::*;
pub use self::key::*;
pub use self::game_state::GameStateCache;
pub use self::database::Database;


use primitives;
/// A unique plot identification marker based on it's (x,y) coordinate.
pub type PlotID = primitives::Coord;