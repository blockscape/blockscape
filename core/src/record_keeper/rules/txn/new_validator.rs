use primitives::Txn;
use record_keeper::{Error, LogicError, NetState};
use record_keeper::rules::TxnRule;
use primitives::Change;
use record_keeper::key::*;

/// The txn creator must be the Admin if it contains a new validator change
pub struct NewValidator;
impl TxnRule for NewValidator {
    fn is_valid(&self, state: &NetState, txn: &Txn) -> Result<(), Error> {
        let mut contains_nv = false;
        for change in txn.mutation.changes.iter() {
            match change {
                &Change::NewValidator {..} => { contains_nv = true; break; },
                _ => ()
            }
        }

        if contains_nv {
            let admin_key_id = state.get_obj(NetworkEntry::AdminKeyID.into())?;
            
            if txn.creator == admin_key_id { Ok(()) }
            else { Err(Error::Logic(LogicError::ExpectedAdmin)) }
        }
        else { Ok(()) }
    }

    fn description(&self) -> &'static str {
        "Txns containing a new validator change must be signed by the admin key."
    }
}