use primitives::Block;
use record_keeper::database::Database;
use record_keeper::{Error, LogicError, NetState};
use record_keeper::rules::BlockRule;
use openssl::pkey::PKey;
use std::error::Error as StdErr;

/// The signature on the block must be by a valid signer and the hash must match the signed hash.
pub struct Signature;
impl BlockRule for Signature {
    fn is_valid(&self, state: &NetState, _db: &Database, block: &Block) -> Result<(), Error> {
        let der = match state.get_validator_key(&block.creator) {
            Ok(k) => k,
            Err(Error::NotFound(..)) => return Err(LogicError::UnrecognizedCreator.into()),
            Err(e) => return Err(e)
        };

        let key = PKey::public_key_from_der(&der)
            .map_err(|e| Error::Deserialize(e.description().into()) )?;
        
        if block.verify_signature(&key) { Ok(()) }
        else { Err(LogicError::InvalidSignature.into()) }
    }

    fn description(&self) -> &'static str {
        "The signature on the block must be by a valid signer and the hash must match the signed hash."
    }
}