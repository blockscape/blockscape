use bin::Bin;
use hash::hash_pub_key;
use primitives::{Change, Mutation};
use record_keeper::{Error, LogicError, NetState, PlotEvents, PlotID};
use record_keeper::events::add_event;
use record_keeper::rules::MutationRule;
use std::collections::{HashSet, HashMap};


/// Make sure there are not duplicate changes in a mutation which are clearly invalid, such as
/// duplicate NewValidator or duplicate PlotEvents
pub struct Duplicates;
impl MutationRule for Duplicates {
    fn is_valid(&self, state: &NetState, mutation: &Mutation, _cache: &mut Bin) -> Result<(), Error> {
        let mut validators = HashSet::new();
        let mut events: HashMap<PlotID, PlotEvents> = HashMap::new();

        for change in mutation.changes.iter() {  match change {
            &Change::NewValidator { ref pub_key } => {
                let hash = hash_pub_key(pub_key);
                if validators.contains(&hash) || state.get_validator_key(hash).is_ok() {
                    // technically the DB could return an error if it fails for other reason, but is
                    // unlikely enough and can rely on other validators to ignore it
                    return Err(LogicError::Duplicate.into());
                }

                validators.insert(hash);
            },
            &Change::Event { id, tick, ref event } => {
                // Check if we have already encountered it in this txn
                if let Some(plot_events) = events.get(&id) {
                    if let Some(event_list) = plot_events.get(&tick) {
                        if event_list.contains(event) {
                            return Err(LogicError::Duplicate.into());
                        }
                    }
                }

                // Check if it is in the network state
                let plot_events = state.get_plot_events(id, tick)?;
                if let Some(event_list) = plot_events.get(&tick) {
                    if event_list.contains(event) {
                        return Err(LogicError::Duplicate.into());
                    }
                }
                
                // Add it to what we have seen
                if let Some(plot_events) = events.get_mut(&id) {
                    add_event(plot_events, tick, event.clone());
                    continue;
                }

                let mut plot_events = PlotEvents::new();
                add_event(&mut plot_events, tick, event.clone());
                events.insert(id, plot_events);
            }
            _ => ()
        }}

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Mutations may not contain duplicate, non-stacking changes."
    }
}