use bincode;
use hash::{hash_obj, merge_hashes};
use primitives::{U256, U256_ZERO};
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::cmp::Ordering;
use time::Time;
use range::Range;

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

impl PartialEq for BlockHeader {
    fn eq(&self, other: &BlockHeader) -> bool {
        self.calculate_hash() == other.calculate_hash()
    }
} impl Eq for BlockHeader {}

impl BlockHeader {
    pub fn calculate_hash(&self) -> U256 {
        hash_obj(self)
    }
}

/// The core unit of the blockchain.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub txns: BTreeSet<U256>,
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

impl PartialOrd for Block {
    fn partial_cmp(&self, other: &Block) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Block {
    fn cmp(&self, other: &Block) -> Ordering {
        let a = self.calculate_hash();
        let b = other.calculate_hash();
        a.cmp(&b)
    }
}

impl Block {
    /// Custom deserialization implementation
    pub fn deserialize(header: BlockHeader, raw_txns: &[u8]) -> Result<Block, bincode::Error> {
        let txns = bincode::deserialize::<BTreeSet<U256>>(raw_txns)?;
        Ok(Block{header, txns})
    }

    /// Calculate the merkle root of a set of transactions.
    pub fn calculate_merkle_root(txn_set: &BTreeSet<U256>) -> U256 {
        // What we want to do, is calculate the hash of each two hashes in series, and then form a
        // list of those, repeat until we end up with a single hash.
        
        let mut hashes: Vec<U256> = txn_set.iter().cloned().collect();
        let mut len = hashes.len();

        while len > 1 {
            let mut h: Vec<U256> = Vec::new();
            
            for i in Range(0, len, 2) {
                h.push( merge_hashes(&hashes[i], &hashes[i+1]) );
            } if (len % 2) == 1 { //if an odd number, we will have a tailing hash we need to include
                h.push(hashes[len - 1])
            }

            hashes = h;
            len = hashes.len();
        }

        if len == 1 {
            hashes[0]
        } else {
            hash_obj(&U256_ZERO)
        }
    }
}
