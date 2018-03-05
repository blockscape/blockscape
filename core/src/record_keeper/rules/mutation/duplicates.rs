use bin::Bin;
use hash::hash_pub_key;
use primitives::{Change, RawEvent, RawEvents, U160};
use record_keeper::{Error, LogicError, NetState, PlotID};
use primitives::add_event;
use record_keeper::rules::MutationRule;
use std::collections::{HashSet, HashMap};


/// Make sure there are not duplicate changes in a mutation which are clearly invalid, such as
/// duplicate NewValidator or duplicate PlotEvents
pub struct Duplicates;
impl Duplicates {
    /// Check if a given event is already in the set of events at the given tick.
    fn duplicated_within_txn(events: &HashMap<PlotID, RawEvents>, id: PlotID, tick: u64, event: &RawEvent) -> bool {
        if let Some(plot_events) = events.get(&id) {
            if let Some(event_list) = plot_events.get(&tick) {
                if event_list.contains(event) {
                    return true;
                }
            }
        } false
    }

    /// Check if a given event is already in the set of events at the given tick in the network
    /// state.
    fn duplicated_within_net(state: &NetState, id: PlotID, tick: u64, event: &RawEvent) -> Result<bool, Error> {
        let plot_events = state.get_plot_events(id, tick)?;
        if let Some(event_list) = plot_events.get(&tick) {
            if event_list.contains(event) {
                return Ok(true);
            }
        } Ok(false)
    }

    /// Put the event into the records as having been seen for a specified plot.
    fn append_to_plot(events: &mut HashMap<PlotID, RawEvents>, id: PlotID, tick: u64, event: RawEvent) {
        if let Some(plot_events) = events.get_mut(&id) {
            add_event(plot_events, tick, event.clone());
            return;
        }

        let mut plot_events = RawEvents::new();
        add_event(&mut plot_events, tick, event.clone());
        events.insert(id, plot_events);
    }
}

impl MutationRule for Duplicates {
    fn is_valid(&self, state: &NetState, mutation: &Vec<(Change, U160)>, _cache: &mut Bin) -> Result<(), Error> {
        let mut validators = HashSet::new();
        let mut events: HashMap<PlotID, RawEvents> = HashMap::new();

        for &(ref change, _) in mutation {  match change {
            &Change::NewValidator { ref pub_key } => {
                let hash = hash_pub_key(pub_key);
                if validators.contains(&hash) || state.get_validator_key(hash).is_ok() {
                    // technically the DB could return an error if it fails for other reason, but is
                    // unlikely enough and can rely on other validators to ignore it
                    return Err(LogicError::Duplicate.into());
                }

                validators.insert(hash);
            },
            &Change::PlotEvent(ref e) => {
                // Check if we have already encountered it in this txn
                if Self::duplicated_within_txn(&events, e.from, e.tick, &e.event) ||
                   e.to.iter()
                    .find(|&&id| Self::duplicated_within_txn(&events, id, e.tick, &e.event))
                    .is_some()
                {
                    return Err(LogicError::Duplicate.into());
                }

                // Check if it is in the network state
                if Self::duplicated_within_net(state, e.from, e.tick, &e.event)? {
                    return Err(LogicError::Duplicate.into());
                } for id in e.to.iter() {
                    if Self::duplicated_within_net(state, *id, e.tick, &e.event)? {
                        return Err(LogicError::Duplicate.into());
                    }
                }
                
                // Add it to what we have seen
                Self::append_to_plot(&mut events, e.from, e.tick, e.event.clone());
                for id in e.to.iter() {
                    Self::append_to_plot(&mut events, *id, e.tick, e.event.clone());
                }
            }
            _ => ()
        }}

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Mutations may not contain duplicate, non-stacking changes."
    }
}