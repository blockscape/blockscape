use blockscape_core::record_keeper::{MutationRule, Error, LogicError, NetState, plot_events_rule_iter};
use blockscape_core::primitives::{Change, U160};
use blockscape_core::bin::*;
use checkers;
use bincode;
use std::collections::HashMap;

/// Enforce tick progression to happen as turns. I.e. tick 0 is init, and then odd ticks are player1's turn and even ticks are player2's turn.
pub struct Turns;
impl MutationRule for Turns {
    fn is_valid(&self, net_state: &NetState, mutation: &Vec<(Change, U160)>, _cache: &mut Bin) -> Result<(), Error> {
        // Construct a list of all the game events so we can mess around with it
        let mut events = Vec::new();
        plot_events_rule_iter(|event, player| {
            events.push((
                event.from,
                event.tick,
                bincode::deserialize::<checkers::Event>(&event.event)?,
                player
            )); Ok(())
        }, mutation)?;

        // sort them by plot and then by tick
        events.sort_by(|a, b|
            a.0.cmp(&b.0).then(
                a.1.cmp(&b.1)
            )
        );

        // keep track of the players of games to prevent needing to find them again
        let mut players= HashMap::new();

        // Very there are no gaps within each Plot's set of events, 0-turn is a new game, actions
        // take place on an existing board, and things do not overlap with the database.
        let mut last_coord = None;
        let mut last_turn = 0u64;
        let mut iter = events.iter();
        while let Some(i) = iter.next() {
            if last_coord == Some(i.0) { // same plot
                if (last_turn + 1) != i.1 {
                    return Err(LogicError::InvalidMutation("Cannot skip or duplicate turns".into()).into())
                } last_turn += 1;
            }
            else { // new plot encountered
                last_coord = Some(i.0);
                last_turn = i.1;

                // check network state to make sure we are continuing in the right place
                if i.1 == 0 { //trying to make a new game
                    if let checkers::Event::Start(p1, p2) = i.2 {
                        players.insert(i.0, (p1, p2));
                    } else {
                        return Err(LogicError::InvalidMutation("Must have new game txn to begin a game".into()).into())
                    }

                    if !net_state.get_plot_events(i.0, 0)?.is_empty() {
                        return Err(LogicError::InvalidMutation("Cannot start a new game on an existing board.".into()).into())
                    }
                }
                else { //trying to continue an existing game
                    let events = net_state.get_plot_events(i.0, 0)?;
                    let ps = events.get(&0);
                    if ps.is_none() || !events.contains_key(&(i.1 - 1)) {
                        return Err(LogicError::InvalidMutation("Missing prior turns.".into()).into())
                    }
                    if events.contains_key(&i.1) {
                        return Err(LogicError::InvalidMutation("Cannot replace existing turn.".into()).into())
                    }
                    if let checkers::Event::Start(p1, p2) = bincode::deserialize(&ps.unwrap()[0])? {
                        players.insert(i.0, (p1, p2));
                    } else { unreachable!() }
                }
            }
        }

        // make sure players play on the correct turn
        for &(plot, tick, _, player) in events.iter() {
            if tick == 0 { continue; }
            let &(p1, p2) = players.get(&plot).unwrap();
            if tick % 2 == 1 { // p1 turn
                if player != p1 {
                    return Err(LogicError::InvalidMutation("Invalid player.".into()).into())
                }
            } else { // p2 turn
                if player != p2 {
                    return Err(LogicError::InvalidMutation("Invalid player.".into()).into())
                }
            }
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Turns must be played in order, by the correct player, and not be duplicated."
    }
}