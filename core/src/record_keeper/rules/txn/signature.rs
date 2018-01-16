use primitives::Txn;
use record_keeper::{Error, LogicError, NetState};
use record_keeper::rules::TxnRule;
use openssl::pkey::PKey;
use std::error::Error as StdErr;

/// The signature on the block must be by a valid signer and the hash must match the signed hash.
pub struct Signature;
impl TxnRule for Signature {
    fn is_valid(&self, state: &NetState, txn: &Txn) -> Result<(), Error> {
        let der = match state.get_validator_key(&txn.creator) {
            Ok(k) => k,
            Err(Error::NotFound(..)) => return Err(LogicError::UnrecognizedCreator.into()),
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