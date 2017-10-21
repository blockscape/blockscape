use primitives::U256;
use std::collections::BTreeSet;
use time::Time;

use hash::hash_obj;

/// The main infromation about a block. This noteably excludes the list of transactions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u16,
    pub timestamp: Time,
    pub hash_previous_block: U256,
    pub hash_merkle_root: U256,
}

impl BlockHeader {
    pub fn calculate_hash(&self) -> U256 {
        hash_obj(self)
    }
}

/// The core unit of the blockchain.
#[derive(Serialize, Deserialize, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: BTreeSet<U256>,
}

pub trait HasBlockHeader {
    fn get_header(&self) -> &BlockHeader;
}


impl HasBlockHeader for BlockHeader {
    fn get_header(&self) -> &BlockHeader {
        &self
    }
}

impl HasBlockHeader for Block {
    fn get_header(&self) -> &BlockHeader {
        &self.header
    }
}


impl Block {
    pub fn calculate_merkle_root(&self) -> U256 {
        unimplemented!("Calculate merkle root has not yet been completed!");
    }
}
