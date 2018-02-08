use blockscape_core::record_keeper::{MutationRule, Error, LogicError, NetState, GameStateCache};
use blockscape_core::primitives::{Change, U160};
use blockscape_core::bin::*;
use std::error::Error as StdErr;
use checkers;
use bincode;
use std::sync::{Arc, RwLock};

use game::GameCache;

/// Validate based on game rules to make sure actions are valid. This will create and track a
/// checkers game state for each plot and verify that a given move make sense and perform it.
pub struct Game(GameCache);

impl Default for Game {
    fn default() -> Game {
        Game(Arc::new(RwLock::new(GameStateCache::new())))
    }
}

impl Game {
    pub fn new(cache: GameCache) -> Game {
        Game(cache)
    }

    pub fn get_cache(&self) -> GameCache {
        Arc::clone(&self.0)
    }
}

impl MutationRule for Game {
    fn is_valid(&self, net_state: &NetState, mutation: &Vec<(Change, U160)>, cache: &mut Bin) -> Result<(), Error> {
        let events = super::get_events(mutation)?;

        let mut cache = self.0.write().unwrap();
        for (event, player) in events {
            let (start_tick, mut board) =
                if event.tick == 0 {
                    // can ignore game setup events here
                    continue;
                } else {
                    // retrieve the board from before this turn
                    cache.latest(event.from, Some(event.tick - 1))
                    .map(|(t, b)| (t, b.clone()))
                    .unwrap_or((0, checkers::Board::default()))
                };

            // Get the board up to our current location (if needed)
            // Should land at one tick prior to `event.tick`
            debug_assert!(start_tick < event.tick);
            if (start_tick + 1) < event.tick {
                // we can assume these will all work because they have been deemed valid already.
                let old_events = net_state.get_plot_events(event.from, start_tick + 1)?;
                for (t, e) in old_events {
                    debug_assert_eq!(e.len(), 1);
                    board.play(
                        bincode::deserialize(&e[0]).unwrap(),
                        checkers::Player::from_turn(t).unwrap()
                    ).unwrap();
                }
            }

            // Test the move
            let player = checkers::Player::from_turn(event.tick).unwrap();
            board.play(event.event, player)
                .map_err(|e| Error::Logic(LogicError::InvalidMutation(e.description().into())))?;

            // Cache the board to reduce computation later.
            cache.cache(event.tick, event.from, board);
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "The rules of checkers must be followed."
    }
}