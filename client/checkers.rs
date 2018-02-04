use blockscape_core::primitives::{U160};
use blockscape_core::primitives::Event as CoreEvent;
use blockscape_core::bin::*;
use bincode;

/// The 4 cardinal directions of checkers game play.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum Direction {
    NW, NE, SE, SW
}

/// The base game events for checkers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Start(U160, U160),
    Move(u8, Direction),
    Jump(u8, Vec<Direction>)
} impl CoreEvent for Event {}

impl AsBin for Event {
    fn as_bin(&self) -> Bin {
        bincode::serialize(self, bincode::Infinite).unwrap()
    }
}
