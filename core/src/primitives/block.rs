use bincode;
use hash::hash_obj;
use primitives::U256;
use std::collections::BTreeSet;
use time::Time;

/// The main infromation about a block. This noteably excludes the list of transactions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u16,
    pub timestamp: Time,
    pub prev: U256,
    pub merkle_root: U256,
}

impl BlockHeader {
    pub fn calculate_hash(&self) -> U256 {
        hash_obj(self)
    }
}

/// The core unit of the blockchain.
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    /// Custom deserialization implementation
    pub fn deserialize(header: BlockHeader, raw_txns: &[u8]) -> Result<Block, bincode::Error> {
        let transactions = bincode::deserialize::<BTreeSet<U256>>(raw_txns)?;
        Ok(Block{header, transactions})
    }

    pub fn calculate_merkle_root(&self) -> U256 {
        unimplemented!("Calculate merkle root has not yet been completed!");
    }
}
