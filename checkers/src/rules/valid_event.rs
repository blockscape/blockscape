use blockscape_core::record_keeper::{MutationRule, Error, LogicError, DBState, plot_events_rule_iter};
use blockscape_core::primitives::{Change, U160};
use blockscape_core::bin::*;
use bincode;
use checkers;

/// Make sure plot events are valid game events. I.e. Verify they can be deserialized.
/// TODO: Determine if we want to keep this because an error would be thrown otherwise, just not a logic error.
pub struct ValidEvent;
impl MutationRule for ValidEvent {
    fn is_valid(&self, _state: &DBState, mutation: &Vec<(Change, U160)>, _cache: &mut Bin) -> Result<(), Error> {
        plot_events_rule_iter(|event, _| {
            if let Err(err) = bincode::deserialize::<checkers::Event>(&event.event) {
                Err(LogicError::InvalidMutation(err.to_string()).into())
            } else { Ok(()) }
        }, mutation)
    }

    fn description(&self) -> &'static str {
        "An event must be deserializable into a valid game event."
    }
}