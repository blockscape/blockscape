use bincode::{serialize, deserialize, Bounded};

use bytes::LittleEndian;
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use std::collections::BTreeSet;
use u256::{U256, U256_ZERO};
use time::Time;


type DefaultByteOrder = LittleEndian;


/// The main infromation about a block. This noteably excludes the list of transactions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u16,
    pub timestamp: Time,
    pub hash_previous_block: U256,
    pub hash_merkle_root: U256,
}

/// The core unit of the blockchain.
#[derive(Serialize, Deserialize)]
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