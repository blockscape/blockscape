use primitives::Block;
use record_keeper::{Error, LogicError, DBState};
use record_keeper::rules::BlockRule;

/// The merkle_root must be correct for the txns included.
pub struct MerkleRoot;
impl BlockRule for MerkleRoot {
    fn is_valid(&self, _prev_state: &DBState, block: &Block) -> Result<(), Error> {
        if block.merkle_root == Block::calculate_merkle_root(&block.txns) {
            Ok(())
        } else {
            Err(LogicError::InvalidMerkleRoot.into())
        }
    }

    fn description(&self) -> &'static str {
        "The merkle root must match the included transactions."
    }
}