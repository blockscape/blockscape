use bincode;
use hash::hash_obj;
use primitives::U256;
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use time::Time;

/// The main infromation about a block. This noteably excludes the list of transactions.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// The version used to make the block, allows for backwards compatibility
    pub version: u16,
    /// The time when the block was created
    pub timestamp: Time,
    /// The shard is the hash of the origin block for the shard, or 0 if it is an origin block
    pub shard: U256,
    /// The previous block in the chain
    pub prev: U256,
    /// Hash identifer of the txn list
    pub merkle_root: U256,
}

impl BlockHeader {
    pub fn calculate_hash(&self) -> U256 {
        hash_obj(self)
    }
}

impl PartialEq for BlockHeader {
    fn eq(&self, other: &BlockHeader) -> bool {
        self.calculate_hash() == other.calculate_hash()
    }
} impl Eq for BlockHeader {}

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

impl Deref for Block {
    type Target = BlockHeader;
    
    fn deref(&self) -> &BlockHeader {
        &self.header
    }
}

impl DerefMut for Block {
    fn deref_mut(&mut self) -> &mut BlockHeader {
        &mut self.header
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Block) -> bool {
        // The header has the merkle root so this is valid
        self.header == other.header
    }
} impl Eq for Block {}

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
