use bin::{Bin, AsBin};
use primitives::{U256, U160};
use super::{PlotID};
use record_keeper::database as DB;

#[inline]
fn prefix<T: AsBin>(p: &[u8], k: &T) -> Bin {
    let mut b = Bin::from(p);
    b.extend_from_slice(&k.as_bin()); b
}


/// Data entries for the blockchain domain
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockchainEntry {
    BlockHeader(U256),
    TxnList(U256),
    Txn(U256)
}

impl AsBin for BlockchainEntry {
    fn as_bin(&self) -> Bin {
        use self::BlockchainEntry::*;
        match self {
            Txn(h) => prefix(b"T", h),
            BlockHeader(h) => prefix(b"B", h),
            TxnList(h) => prefix(b"L", h)
        }
    }
}


/// Data entries for the cache domain
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheEntry {
    BlocksByHeight(u64),
    HeightByBlock(U256),
    BlocksByTxn(U256),
    TxnsByAccount(U160),
    TxnReceiveTime(U256),
    ContraMut(U256),
    CurrentHead
}

impl AsBin for CacheEntry {
    fn as_bin(&self) -> Bin {
        use self::CacheEntry::*;
        match self {
            BlocksByHeight(h) => prefix(b"HGT", h),
            HeightByBlock(b) => prefix(b"BHT", b),
            BlocksByTxn(h) => prefix(b"TBK", h),
            TxnsByAccount(h) => prefix(b"ATN", h),
            TxnReceiveTime(h) => prefix(b"RCT", h),
            ContraMut(b) => prefix(b"CMT", b),
            CurrentHead => Bin::from(b"CHead" as &[u8])
        }
    }
}


/// Network entries for the network domain
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetworkEntry {
    Plot(PlotID, u64),
    ValidatorKey(U160),
    ValidatorStake(U160),
    AdminKeyID,
    Generic(Bin)
}

impl AsBin for NetworkEntry {
    fn as_bin(&self) -> Bin {
        use self::NetworkEntry::*;
        match self {
            Plot(id, tick) => prefix(&prefix(b"PLT", id), &(tick / DB::PLOT_EVENT_BUCKET_SIZE)),
            ValidatorKey(k) => prefix(b"VKY", k),
            ValidatorStake(k) => prefix(b"VSK", k),
            AdminKeyID => Bin::from(b"ADMIN" as &[u8]),
            Generic(b) => b.clone()
        }
    }
}


/// A database key which is designed to clearly and uniquely identify an entry in the database. The
/// separate domains in the database; one for chainstate, networkstate and cachestate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Blockchain(BlockchainEntry),
    Cache(CacheEntry),
    Network(NetworkEntry)
}

impl AsBin for Key {
    fn as_bin(&self) -> Bin {
        use self::Key::*;
        match self {
            Blockchain(e) => prefix(b"b", e),
            Cache(e)      => prefix(b"c", e),
            Network(e)    => prefix(b"n", e)
        }
    }
}

impl From<BlockchainEntry> for Key {
    fn from(e: BlockchainEntry) -> Self {
        Key::Blockchain(e)
    }
}

impl From<CacheEntry> for Key {
    fn from(e: CacheEntry) -> Self {
        Key::Cache(e)
    }
}

impl From<NetworkEntry> for Key {
    fn from(e: NetworkEntry) -> Self {
        Key::Network(e)
    }
}