use bin::Bin;
use primitives::{Change, U160};
use record_keeper::{Error, LogicError, NetState};
use record_keeper::PlotEvent as PE;
use record_keeper::rules::MutationRule;
use record_keeper::rules::plot_events_rule_iter;


/// Make sure PlotEvent Changes in the mutation seem valid. (Cannot verify the RawEvent though)
pub struct PlotEvent;
impl PlotEvent {
    fn validate(e: &PE, _: U160) -> Result<(), Error> {
        // TODO: verify all the PlotIDs exist

        if e.to.contains(&e.from) {
            Err(Error::Logic(LogicError::InvalidMutation("PlotEvent changes must not include themselves in the recipient list.".into())))
        } else { Ok(())}

        // TODO: Make sure the tick is not past a threshold?
    }
}

impl MutationRule for PlotEvent {
    fn is_valid(&self, _state: &NetState, mutation: &Vec<(Change, U160)>, _cache: &mut Bin) -> Result<(), Error> {
        plot_events_rule_iter(Self::validate, mutation)
    }

    fn description(&self) -> &'static str {
        "PlotEvents must have valid information."
    }
}