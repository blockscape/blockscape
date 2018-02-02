use bin::Bin;
use primitives::{Change, Mutation};
use record_keeper::{Error, LogicError, NetState};
use record_keeper::rules::MutationRule;


/// Make sure PlotEvent Changes in the mutation seem valid. (Cannot verify the RawEvent though)
pub struct PlotEvent;
impl MutationRule for PlotEvent {
    fn is_valid(&self, _state: &NetState, mutation: &Mutation, _cache: &mut Bin) -> Result<(), Error> {
        for change in mutation.changes.iter() {  match change {
            &Change::PlotEvent(ref e) => {
                // TODO: verify all the PlotIDs exist
                
                if e.to.contains(&e.from) {
                    return Err(Error::Logic(LogicError::InvalidMutation("PlotEvent changes must not include themselves in the recipient list.".into())))
                }

                // TODO: Make sure the tick is not past a threshold?
            }
            _ => ()
        }}

        Ok(())
    }

    fn description(&self) -> &'static str {
        "PlotEvents must have valid information."
    }
}