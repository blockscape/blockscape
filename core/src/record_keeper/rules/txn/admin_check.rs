use primitives::Txn;
use record_keeper::{Error, LogicError, DBState};
use record_keeper::rules::TxnRule;
use primitives::Change;
use record_keeper::key::*;

/// The txn creator must be the Admin if it contains an admin change
pub struct AdminCheck;
impl TxnRule for AdminCheck {
    fn is_valid(&self, state: &DBState, txn: &Txn) -> Result<(), Error> {
        let mut contains_admin = false;
        for change in txn.mutation.changes.iter() {
            match change {
                &Change::Admin {..} => { contains_admin = true; break; },
                _ => ()
            }
        }

        if contains_admin {
            let admin_key_id = state.get_obj(NetworkEntry::AdminKeyID.into())?;
            
            if txn.creator == admin_key_id { Ok(()) }
            else { Err(Error::Logic(LogicError::ExpectedAdmin)) }
        }
        else { Ok(()) }
    }

    fn description(&self) -> &'static str {
        "Txns containing an Admin change must be signed by the admin key."
    }
}