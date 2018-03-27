pub mod block_package;
pub mod database;
pub mod error;
pub mod events;
pub mod key;
pub mod net_dif;
pub mod net_state;
pub mod game_state;

mod record_keeper;
mod rules;

pub use self::block_package::BlockPackage;
pub use self::error::*;
pub use self::events::{PlotEvent, JPlotEvent, RecordEvent, DePlotEvent};
pub use self::net_dif::*;
pub use self::net_state::*;
pub use self::record_keeper::*;
pub use self::rules::*;
pub use self::key::*;
pub use self::game_state::GameStateCache;


use primitives;
/// A unique plot identification marker based on it's (x,y) coordinate.
pub type PlotID = primitives::Coord;