use bincode;
use crypto::digest::Digest;
use crypto::ripemd160::Ripemd160;
use crypto::sha3::Sha3;
use serde::Serialize;

use primitives::{U256, U160};

/// Hash bytes of data and then return the result as a U256.
/// This uses a double sha3-256 hash.
pub fn hash_bytes(bytes: &[u8]) -> U256 {
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
    let encoded: Vec<u8> = bincode::serialize(obj, bincode::Infinite).unwrap();
    hash_bytes(&encoded)
}

/// Calculate the hash of two hashes. Note: merge_hashes(a, b) != merge_hashes(b, a)
pub fn merge_hashes(a: &U256, b: &U256) -> U256 {
    let mut m: Vec<u8> = bincode::serialize(a, bincode::Bounded(32)).unwrap();
    m.extend_from_slice( &bincode::serialize(b, bincode::Bounded(32)).unwrap() );
    hash_bytes(&m)
}

#[test]
fn test_hash_bytes() {
    let buf_in : Vec<u8> = Vec::new();
    // Generated with hashlib for python
    // h1, h2 = hashlib.sha3_256(), hashlib.sha3_256()
    // h2.update(h1.digest())
    // h2.hexdigest()
    let expected = U256::from_big_endian(&[ 0xa1, 0x29, 0x2c, 0x11, 0xcc, 0xdb, 0x87, 0x65,
                                            0x35, 0xc6, 0x69, 0x9e, 0x82, 0x17, 0xe1, 0xa1,
                                            0x29, 0x41, 0x90, 0xd8, 0x3e, 0x42, 0x33, 0xec,
                                            0xc4, 0x90, 0xd3, 0x2d, 0xf1, 0x7a, 0x41, 0x16]);
    
    let actual = hash_bytes(&buf_in);
    assert_eq!(expected, actual);
}

#[test]
fn test_hash_pub_key() {
    let buf_in : Vec<u8> = Vec::new();
    // Generated with hashlib for python
    // h1, h2 = hashlib.sha3_256(), hashlib.new('ripemd160')
    // h2.update(h1.digest())
    // h2.hexdigest()
    let expected = U160::from_big_endian(&[0xb0, 0xa2, 0xc9, 0x10, 0x8b, 0x9c, 0xff, 0x7f,
                                        0x0f, 0x68, 0x6f, 0xef, 0x1d, 0x2e, 0xcb, 0xd5,
                                        0xf1, 0x99, 0x99, 0x72]);
    let actual = hash_pub_key(&buf_in);
    assert_eq!(expected, actual);
}