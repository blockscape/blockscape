use std::vec::Vec;
use u256::U256;

// Expand and divide shard transactions
// Positive repuation transaction ?
// Slashing transaction
// Superblocks store hashes of sublocks current subchain blocks
// Any mutations to game state
// Transfers between shards must be signed by a bunch of people

#[derive(Serialize, Deserialize)]
/// Represents a Transaction on the network.
pub struct Txn {
    pub timestamp: u64,
    pub txn_type: u8,
    pub pubkey: U256,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}

impl Txn {
    
}