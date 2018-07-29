use primitives::Txn;
use record_keeper::{Error, DBState};
use record_keeper::rules::{TxnRule, MutationRule};

use record_keeper::rules::mutation::Duplicates as MutationDuplicates;


/// Make sure there are not duplicate changes in a txn which are clearly invalid, such as duplicate
/// NewValidator or duplicate PlotEvents
pub struct Duplicates;
impl TxnRule for Duplicates {
    fn is_valid(&self, state: &DBState, txn: &Txn) -> Result<(), Error> {
        let mut mutation = Vec::new();
        for change in txn.mutation.changes.iter().cloned() {
            mutation.push((change, txn.creator));
        }
        let mut stupid = Vec::new();  // will not be used...
        MutationDuplicates.is_valid(state, &mutation, &mut stupid)
    }

    fn description(&self) -> &'static str {
        "Txns may not contain duplicate, non-stacking changes."
    }
}