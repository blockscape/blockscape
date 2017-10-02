use u256::U256;
use u160::U160;

use crypto::digest::Digest;
use crypto::sha3::Sha3;
use crypto::ripemd160::Ripemd160;

use openssl::pkey::PKey;

use serde::Serialize;
use bincode;

/// Hash bytes of data and then return the result as a U256.
/// This uses a double sha3-256 hash.
fn hash_bytes(bytes: &[u8]) -> U256 {
    let mut buf = [0u8; 32];
    let mut hasher = Sha3::sha3_256();

    hasher.input(bytes);
    hasher.result(&mut buf); //don't care about first hash, only the second
    hasher.reset();

    hasher.input(&buf);
    hasher.result(&mut buf);
    U256::from_big_endian(&mut buf)
}

/// Hash a public key and return the result as a U160. This uses the SHA3-256
/// hashing function followed by the Ripemd-160 hashing function.
pub fn hash_pub_key(bytes: &[u8]) -> U160 {
    let mut buf = [0u8; 32];
    let mut hasher1 = Sha3::sha3_256();
    let mut hasher2 = Ripemd160::new();
    
    hasher1.input(bytes);
    hasher1.result(&mut buf);
    hasher2.input(&buf);
    hasher2.result(&mut buf);
    U160::from_big_endian(&mut buf)
}

/// Hash a serilizable object by serialzing it with bincode, and then hashing the bytes.
/// This uses a doulbe sha3-256 hash.
pub fn hash_obj<S: Serialize>(obj: &S) -> U256 {
    let encoded : Vec<u8> = bincode::serialize(&obj, bincode::Infinite).unwrap();
    hash_bytes(&encoded)
}