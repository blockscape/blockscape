use blockscape_core::record_keeper::{MutationRules, Error, PlotID, plot_events_rule_iter, DePlotEvent};
use blockscape_core::primitives::{Change, U160};
use bincode;
use checkers;

mod valid_event;
mod turns;
mod game;

pub fn build_rules() -> MutationRules {
    let mut rules = MutationRules::new();
    rules.push_back(Box::new(valid_event::ValidEvent));
    rules.push_back(Box::new(turns::Turns));
    rules.push_back(Box::new(game::Game::default()));
    rules
}

/// Construct a list of game events by processing the list of changes in the mutation and sort them
/// by (`from`) plot id followed by tick.
fn get_events(mutation: &Vec<(Change, U160)>) -> Result<Vec<(DePlotEvent<checkers::Event>, U160)>, Error> {
    let mut events = Vec::new();
    plot_events_rule_iter(|event, player| {
        events.push((DePlotEvent::deserialize(event)?, player)); Ok(())
    }, mutation)?;

    // sort them by plot and then by tick
    events.sort_by(|a, b|
        a.0.from.cmp(&b.0.from).then(
            a.0.tick.cmp(&b.0.tick)
        )
    );

    Ok(events)
}