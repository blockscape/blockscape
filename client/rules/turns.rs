use blockscape_core::record_keeper::{MutationRule, Error, NetState, LogicError};
use blockscape_core::record_keeper::error::assert_mut_valid;
use blockscape_core::primitives::{Change, U160};
use blockscape_core::bin::*;
use checkers;
use bincode;
use std::collections::HashMap;

/// Enforce tick progression to happen as turns. I.e. tick 0 is init, and then odd ticks are
/// player1's turn and even ticks are player2's turn. This does not validate game logic, only
/// blockchain-level stuff which must be correct.
pub struct Turns;
impl MutationRule for Turns {
    fn is_valid(&self, net_state: &NetState, mutation: &Vec<(Change, U160)>, _cache: &mut Bin) -> Result<(), Error> {
        // Construct a list of all the game events so we can mess around with it
        let events = super::get_events(mutation)?;

        // keep track of the players of games to prevent needing to find them again
        let mut players= HashMap::new();

        // Very there are no gaps within each Plot's set of events, 0-turn is a new game, actions
        // take place on an existing board, and things do not overlap with the database.
        let mut last_coord = None;
        let mut last_turn = 0u64;
        let mut iter = events.iter();
        while let Some(&(ref e, _)) = iter.next() {
            assert_mut_valid(e.tick <= 500, "Games may not have more than 500 turns.")?;
            assert_mut_valid(e.to.is_empty(), "Checkers events only occur on one plot.")?;

            if last_coord == Some(e.from) { // same plot
                assert_mut_valid(last_turn + 1 == e.tick, "Cannot skip or duplicate turns.")?;
                last_turn += 1;
            }
            else { // new plot encountered
                last_coord = Some(e.from);
                last_turn = e.tick;

                // check network state to make sure we are continuing in the right place
                if e.tick == 0 { //trying to make a new game
                    if let checkers::Event::Start(p1, p2) = e.event {
                        players.insert(e.from, (p1, p2));
                    } else {
                        return Err(LogicError::InvalidMutation("Must have new game txn to begin a game".into()).into())
                    }

                    assert_mut_valid(
                        net_state.get_plot_events(e.from, 0)?.is_empty(),
                        "Cannot start a new game on an existing board."
                    )?;
                }
                else { //trying to continue an existing game
                    let events = net_state.get_plot_events(e.from, 0)?;
                    let ps = events.get(&0);
                    assert_mut_valid(!ps.is_none() && events.contains_key(&(e.tick - 1)), "Missing prior turns.")?;
                    assert_mut_valid(!events.contains_key(&e.tick), "Cannot replace existing turn.")?;
                    if let checkers::Event::Start(p1, p2) = bincode::deserialize(&ps.unwrap()[0])? {
                        players.insert(e.from, (p1, p2));
                    } else { unreachable!() }
                }
            }
        }

        // make sure players play on the correct turn
        for &(ref e, player) in events.iter() {
            if e.tick == 0 { continue; }
            let &(p1, p2) = players.get(&e.from).unwrap();
            if e.tick % 2 == 1 { // p1 turn
                assert_mut_valid(player == p1, "Invalid player.")?;
            } else { // p2 turn
                assert_mut_valid(player == p2, "Invalid player.")?;
            }
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Turns must be played in order, by the correct player, and not be duplicated."
    }
}