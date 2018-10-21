use bin::{Bin, JBin};
use hash::{hash_obj, merge_hashes};
use primitives::{U256, JU256, U256_ZERO};
use range::Range;
use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};
use time::Time;

/// The main infromation about a block. This noteably excludes the list of transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Binary blob of data which can be used to save things such as difficulty
    pub blob: Bin
}

impl PartialEq for BlockHeader {
    fn eq(&self, other: &BlockHeader) -> bool {
        self.calculate_hash() == other.calculate_hash()
    }
} impl Eq for BlockHeader {}

impl BlockHeader {
    /// Calculate the hash of this block by hashing all data in the header.
    pub fn calculate_hash(&self) -> U256 {
        hash_obj(self)
    }
}


/// The core unit of the blockchain.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub txns: Vec<U256>,
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
    /// Calculate the merkle root of a set of transactions.
    pub fn calculate_merkle_root(txn_set: &Vec<U256>) -> U256 {
        // What we want to do, is calculate the hash of each two hashes in series, and then form a
        // list of those, repeat until we end up with a single hash.
        
        let mut hashes: Vec<U256> = txn_set.iter().cloned().collect();
        let mut len = hashes.len();

        while len > 1 {
            let mut h: Vec<U256> = Vec::new();
            
            for i in Range(0, len - 1, 2) {
                h.push( merge_hashes(&hashes[i], &hashes[i+1]) );
            } if (len % 2) == 1 { //if an odd number, we will have a tailing hash we need to include
                h.push(hashes[len - 1])
            }

            hashes = h;
            len = hashes.len();
        }

        if len == 1 { hashes[0] }
        else { U256_ZERO }
    }
}



#[derive(Serialize, Deserialize)]
pub struct JBlockHeader {
    version: u16,
    timestamp: Time,
    shard: JU256,
    prev: JU256,
    merkle_root: JU256,
    blob: JBin
}

#[derive(Serialize, Deserialize)]
pub struct JBlock {
    header: JBlockHeader,
    txns: Vec<JU256>
}

impl From<BlockHeader> for JBlockHeader {
    fn from(h: BlockHeader) -> JBlockHeader {
        JBlockHeader {
            version: h.version,
            timestamp: h.timestamp,
            shard: h.shard.into(),
            prev: h.prev.into(),
            merkle_root: h.merkle_root.into(),
            blob: h.blob.into()
        }
    }
}

impl Into<BlockHeader> for JBlockHeader {
    fn into(self) -> BlockHeader {
        BlockHeader {
            version: self.version,
            timestamp: self.timestamp,
            shard: self.shard.into(),
            prev: self.prev.into(),
            merkle_root: self.merkle_root.into(),
            blob: self.blob.into()
        }
    }
}

impl From<Block> for JBlock {
    fn from(h: Block) -> JBlock {
        JBlock {
            header: h.header.into(),
            txns: h.txns.into_iter().map(|h| h.into()).collect()
        }
    }
}

impl Into<Block> for JBlock {
    fn into(self) -> Block {
        Block {
            header: self.header.into(),
            txns: self.txns.into_iter().map(|h| h.into()).collect()
        }
    }
}

#[test]
fn calculate_merkle_root() {
    
    let mut bts = Vec::new();
    // 0 txns
    assert_eq!(Block::calculate_merkle_root(&bts), U256_ZERO);
    
    bts.push(U256::from(32));
    // 1 txn
    assert_eq!(Block::calculate_merkle_root(&bts), U256::from(32));
    // 2 txns
    bts.push(U256_ZERO);
    let of2 = Block::calculate_merkle_root(&bts);
    assert_ne!(of2, U256::from(32));
    assert_ne!(of2, U256_ZERO);
    bts.push(U256::from(100));
    // 3 txns
    let of3 = Block::calculate_merkle_root(&bts);
    assert_ne!(of3, of2);
    bts.push(U256::from(200));
    // 4 txns
    let of4 = Block::calculate_merkle_root(&bts);
    assert_ne!(of4, of3);
    assert_ne!(of4, of2);
    bts.push(U256::from(300));
    // 5 txns
    let of5 = Block::calculate_merkle_root(&bts);
    assert_ne!(of5, of4);
}
