use u256::U256;
use std::collections::BTreeSet;
use crypto::sha3::Sha3;
use crypto::digest::Digest;
use bytes::{BytesMut, BufMut, LittleEndian};


type DefaultByteOrder = LittleEndian;


/// The main infromation about a block. This noteably excludes the list of transactions.
#[derive(Copy, Clone, PartialEq, Eq)]
struct BlockHeader {
    pub version: u16,
    pub timestamp: u64,
    pub hash_previous_block: U256,
    pub hash_merkle_root: U256,
}

/// The core unit of the blockchain.
struct Block {
    pub header: BlockHeader,
    pub transactions: BTreeSet<U256>,
}



trait HasBlockHeader {
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
    fn hash(&self) -> U256 {
        unimplemented!("Hash has not yet been completed");
        // let mut raw: [u8; 80] = [0; 80];
        let mut raw = BytesMut::with_capacity(80);
        raw.put_u16::<DefaultByteOrder>(self.header.version);
        raw.put_u64::<DefaultByteOrder>(self.header.timestamp);
        //TODO: somehow write U256 values to the buffer
        //TODO: calculate merkeyl tree of transactions

        let mut hasher = Sha3::sha3_256();

        assert!(raw.capacity() >= 8);
        unsafe {
            hasher.input(raw.bytes_mut());
            hasher.result(raw.bytes_mut());
        }

        U256::from(&raw[0..8])
    }
}