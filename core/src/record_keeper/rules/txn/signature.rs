use primitives::Txn;
use record_keeper::{Error, LogicError, DBState};
use record_keeper::rules::TxnRule;
use openssl::pkey::PKey;
use std::error::Error as StdErr;
use primitives::Change;

/// The signature on the txn must be by a valid signer and the hash must match the signed hash.
pub struct Signature;
impl TxnRule for Signature {
    fn is_valid(&self, state: &DBState, txn: &Txn) -> Result<(), Error> {
        let der = match state.get_validator_key(txn.creator) {
            Ok(k) => k,
            Err(Error::NotFound(..)) => {
                // check if new validator and they signed it themselves, otherwise invalid
                if txn.mutation.changes.len() == 1 {
                    if let Change::NewValidator{ref pub_key} = txn.mutation.changes[0] {
                        pub_key.clone()
                    } else { return Err(LogicError::UnrecognizedCreator.into()) }
                } else { return Err(LogicError::UnrecognizedCreator.into()) }
            },
            Err(e) => return Err(e)
        };

        let key = PKey::public_key_from_der(&der)
            .map_err(|e| Error::Deserialize(e.description().into()) )?;
        
        if txn.verify_signature(&key) { Ok(()) }
        else { Err(LogicError::InvalidSignature.into()) }
    }

    fn description(&self) -> &'static str {
        "The signature on the transaction must be by a valid signer and the hash must match the signed hash."
    }
}