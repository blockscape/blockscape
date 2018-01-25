use primitives::Txn;
use record_keeper::{Error, NetState};
use record_keeper::rules::{TxnRule, MutationRule};

use record_keeper::rules::mutation::Duplicates as MutationDuplicates;


/// Make sure there are not duplicate changes in a txn which are clearly invalid, such as duplicate
/// NewValidator or duplicate PlotEvents
pub struct Duplicates;
impl TxnRule for Duplicates {
    fn is_valid(&self, state: &NetState, txn: &Txn) -> Result<(), Error> {
        let mut stupid = Vec::new();  // will not be used...
        MutationDuplicates.is_valid(state, &txn.mutation, &mut stupid)
    }

    fn description(&self) -> &'static str {
        "Txns may not contain duplicate, non-stacking changes."
    }
}