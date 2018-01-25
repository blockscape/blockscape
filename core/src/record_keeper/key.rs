use bin::{Bin, AsBin};
use primitives::{U256, U160};
use super::{PlotID};

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
            &Txn(ref h) => prefix(b"T", h),
            &BlockHeader(ref h) => prefix(b"B", h),
            &TxnList(ref h) => prefix(b"L", h)
        }
    }
}

impl Into<Key> for BlockchainEntry {
    fn into(self) -> Key {
        Key::Blockchain(self)
    }
}


/// Data entries for the cache domain
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheEntry {
    BlocksByHeight(u64),
    HeightByBlock(U256),
    ContraMut(U256),
    CurrentHead
}

impl AsBin for CacheEntry {
    fn as_bin(&self) -> Bin {
        use self::CacheEntry::*;
        match self {
            &BlocksByHeight(ref h) => prefix(b"HGT", h),
            &HeightByBlock(ref b) => prefix(b"BHT", b),
            &ContraMut(ref b) => prefix(b"CMT", b),
            &CurrentHead => Bin::from(b"CHead" as &[u8])
        }
    }
}

impl Into<Key> for CacheEntry {
    fn into(self) -> Key {
        Key::Cache(self)
    }
}


/// Network entries for the network domain
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetworkEntry {
    Plot(PlotID),
    ValidatorKey(U160),
    ValidatorRep(U160),
    AdminKeyID,
    Generic(Bin)
}

impl AsBin for NetworkEntry {
    fn as_bin(&self) -> Bin {
        use self::NetworkEntry::*;
        match self {
            &Plot(ref id) => prefix(b"PLT", id),
            &ValidatorKey(ref k) => prefix(b"VKY", k),
            &ValidatorRep(ref k) => prefix(b"VRP", k),
            &AdminKeyID => Bin::from(b"ADMIN" as &[u8]),
            &Generic(ref b) => b.clone()
        }
    }
}

impl Into<Key> for NetworkEntry {
    fn into(self) -> Key {
        Key::Network(self)
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
            &Blockchain(ref e) => prefix(b"b", e),
            &Cache(ref e)      => prefix(b"c", e),
            &Network(ref e)    => prefix(b"n", e)
        }
    }
}



// A database key, comprised of (prefix, key-value, postfix)
// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub struct Key(pub Option<&'static [u8]>, pub Bin, pub &'static [u8]);
// impl Key {
//     /// Add a prefix to raw data.
//     #[inline]
//     pub fn with_prefix(prefix: &'static [u8], data: &[u8]) -> Vec<u8> {
//         let mut t = Vec::from(prefix);
//         t.extend_from_slice(data); t
//     }

//     /// Add a postfix to raw data
//     #[inline]
//     pub fn with_postfix(data: &[u8], postfix: &'static [u8]) -> Vec<u8> {
//         let mut t = Vec::from(data);
//         t.extend_from_slice(postfix); t
//     }

//     /// Add a prefix and postfix to raw data.
//     #[inline]
//     pub fn with_pre_post_fix(prefix: &'static [u8], data: &[u8], postfix: &'static [u8]) -> Vec<u8> {
//         let mut t = Vec::from(prefix);
//         t.extend_from_slice(data);
//         t.extend_from_slice(postfix); t
//     }
// }

// impl AsBin for Key {
//     fn as_bin(&self) -> Bin {
//         if let Some(pre) = self.0 {
//             Self::with_pre_post_fix(pre, &self.1, self.2)
//         } else {
//             Self::with_postfix(&self.1, self.2)
//         }
//     }
// }