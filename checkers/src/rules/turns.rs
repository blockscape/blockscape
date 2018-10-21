use blockscape_core::record_keeper::{MutationRule, Error, DBState, LogicError, Database};
use blockscape_core::record_keeper::error::assert_mut_valid;
use blockscape_core::primitives::{Change, U160, U160_ZERO};
use blockscape_core::bin::*;
use checkers;
use bincode;
use std::collections::HashMap;

/// Enforce tick progression to happen as turns. I.e. tick 0 is init, and then odd ticks are
/// player1's turn and even ticks are player2's turn. This does not validate game logic, only
/// blockchain-level stuff which must be correct.
pub struct Turns;
impl MutationRule for Turns {
    fn is_valid(&self, state: &DBState, mutation: &Vec<(Change, U160)>, _cache: &mut Bin) -> Result<(), Error> {
        // Construct a list of all the game events so we can mess around with it
        let events = super::get_events(mutation)?;

        // keep track of the players of games to prevent needing to find them again
        let mut boards = HashMap::new();

        // Very there are no gaps within each Plot's set of events, 0-turn is a new game, actions
        // take place on an existing board, and things do not overlap with the database.
        let mut iter = events.iter();
        while let Some(&(ref e, player)) = iter.next() {
            assert_mut_valid(e.tick <= 500, "Games may not have more than 500 turns.")?;
            assert_mut_valid(e.to.is_empty(), "Checkers events only occur on one plot.")?;
            
            
            // check network state to make sure we are continuing in the right place
            if e.tick == 0 { //trying to make a new game
                if let checkers::Event::Start(p1, p2) = e.event {
                    assert_mut_valid(
                        state.get_plot_events(e.from, 0)?.is_empty(),
                        "Cannot start a new game on an existing board."
                    )?;
                    
                    assert_mut_valid(
                        p1 == player || p2 == player,
                        "Cannot start a game for other players."
                    )?;
                    
                    boards.insert(e.from, ((p1, p2), e.tick));
                }
                else {
                    return Err(LogicError::InvalidMutation("Must have new game txn to begin a game".into()).into())
                }
            }
            else if e.tick == 1 {
                if let checkers::Event::Join(player) = e.event {
                    let (p1, p2) = if let Some((players, _)) = boards.remove(&e.from) {
                        players
                    }
                    else {
                        let events = state.get_plot_events(e.from, 0)?;
                        let start_event = events.get(&0);
                        assert_mut_valid(start_event.is_some(), "Missing start")?;
                        assert_mut_valid(!events.contains_key(&1), "Game is already full")?;
                        
                        // the prior event must also be a start event
                        if let checkers::Event::Start(p1, p2) = bincode::deserialize(&start_event.unwrap()[0])? {
                            (p1, p2)
                        }
                        else {
                            unreachable!();
                        }
                    };
                    
                    // reset the players
                    if p1 == U160_ZERO {
                        boards.insert(e.from, ((player, p2), e.tick));
                    }
                    else if p2 == U160_ZERO {
                        boards.insert(e.from, ((p1, player), e.tick));
                    }
                    else {
                        return Err(LogicError::InvalidMutation("Game is already full".into()).into())
                    }
                    
                    
                }
                else {
                    return Err(LogicError::InvalidMutation("Must have new game txn to begin a game".into()).into())
                }
            }
            else {
                
                if let Some((players, last_turn)) = boards.remove(&e.from) { // same plot
                    // either the turn is incremented or it is the first tick and a join has been executed
                    let mut is_join_event = false;
                    if let checkers::Event::Join(..) = e.event {
                        is_join_event = true;
                    }
                    
                    assert_mut_valid(last_turn + 1 == e.tick || 
                        (e.tick == 0 && is_join_event), "Cannot skip or duplicate turns.")?;
                    
                    boards.insert(e.from, (players, e.tick));
                }
                else { // new plot encountered
                    
                    let events = state.get_plot_events(e.from, 0)?;
                    let start_event = events.get(&0);
                    let join_event = events.get(&1);
                    assert_mut_valid(!start_event.is_none(), "Game is not started.")?;
                    
                    assert_mut_valid(e.tick == 2 || events.contains_key(&(e.tick - 1)), "Missing prior turns.")?;
                    assert_mut_valid(!events.contains_key(&e.tick), "Cannot replace existing turn.")?;
                    
                    // cache player information
                    let start_event = start_event.unwrap();
                    if let checkers::Event::Start(p1, p2) = bincode::deserialize(&start_event[0])? {
                        // handle join
                        if join_event.is_some() {
                            if let checkers::Event::Join(player) = bincode::deserialize(&join_event.unwrap()[0])? {
                                if p1 == U160_ZERO {
                                    boards.insert(e.from, ((player, p2), e.tick));
                                }
                                else if p2 == U160_ZERO {
                                    boards.insert(e.from, ((p1, player), e.tick));
                                }
                            }
                            else { unreachable!(); }
                        }
                        else {
                            boards.insert(e.from, ((p1, p2), e.tick));
                        }
                    } else { unreachable!() }
                }
            }
        }

        // make sure players play on the correct turn
        for &(ref e, player) in events.iter() {
            if e.tick == 0 { continue; }
            let &((p1, p2), _) = boards.get(&e.from).unwrap();
            assert_mut_valid(p1 != U160_ZERO && p2 != U160_ZERO, "Waiting for player.")?;
            if e.tick % 2 == 0 { // p1 turn
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
