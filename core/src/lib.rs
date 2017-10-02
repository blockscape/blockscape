extern crate bincode;
extern crate bytes;
extern crate crypto;
extern crate openssl;
extern crate serde_json;
extern crate serde;
extern crate rand;
extern crate time as timelib;
extern crate rocksdb;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

pub mod block;
pub mod txn;
pub mod mutation;
pub mod network;
pub mod u256;
pub mod u160;
pub mod time;
pub mod env;
pub mod database;

use crypto::digest::Digest;
use crypto::sha3::Sha3;
use crypto::ripemd160::Ripemd160;
use openssl::{sign, pkey, hash};
use pkey::PKey;
use serde::Serialize;
use u256::U256;
use u160::U160;

/// The size of any new RSA Keys; other sizes should still be supported.
const RSA_KEY_SIZE: usize = 4096;

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
fn hash_pub_key(bytes: &[u8]) -> U160 {
    let mut buf = [0u8; 32];
    let mut hasher1 = Sha3::sha3_256();
    let mut hasher2 = Ripemd160::new();
    
    hasher1.input(bytes);
    hasher1.result(&mut buf);
    hasher2.input(&buf);
    hasher2.result(&mut buf);
    U160::from_big_endian(&mut buf)
}

/// Sign some bytes with a private key.
fn sign_bytes(bytes: &[u8], private_key: &PKey) -> Vec<u8> {
    let mut signer = sign::Signer::new(hash::MessageDigest::sha256(), &private_key).unwrap();
    signer.update(bytes).unwrap();
    signer.finish().unwrap()
}

/// Verify the bytes have not been tampered with given a signature and public key.
fn verify_bytes(bytes: &[u8], signature: &[u8], public_key: &PKey) -> bool {
    let mut verifier = sign::Verifier::new(hash::MessageDigest::sha256(), public_key).unwrap();
    verifier.update(bytes).unwrap();
    verifier.finish(&signature).unwrap()
}

/// Hash a serilizable object by serialzing it with bincode, and then hashing the bytes.
/// This uses a doulbe sha3-256 hash.
fn hash_obj<S: Serialize>(obj: &S) -> U256 {
    let encoded : Vec<u8> = bincode::serialize(&obj, bincode::Infinite).unwrap();
    hash_bytes(&encoded)
}

/// Sign an object with a private key.
fn sign_obj<S: Serialize>(obj: &S, private_key: &PKey) -> Vec<u8> {
    let encoded: Vec<u8> = bincode::serialize(&obj, bincode::Infinite).unwrap();
    sign_bytes(&encoded, private_key)
}

/// Verify the object has not been tampered with given the signature and public key.
fn verify_obj<S: Serialize>(obj: &S, signature: &[u8], public_key: &PKey) -> bool {
    let encoded: Vec<u8> = bincode::serialize(&obj, bincode::Infinite).unwrap();
    verify_bytes(&encoded, signature, public_key)
}



#[cfg(test)]
mod test {
    use super::{U256, U160, PKey};
    use openssl;

    #[test]
    fn hash_bytes() {
        let buf_in : Vec<u8> = Vec::new();
        // Generated with hashlib for python
        // h1, h2 = hashlib.sha3_256(), hashlib.sha3_256()
        // h2.update(h1.digest())
        // h2.hexdigest()
        let expected = U256::from_big_endian(&[ 0xa1, 0x29, 0x2c, 0x11, 0xcc, 0xdb, 0x87, 0x65,
                                                0x35, 0xc6, 0x69, 0x9e, 0x82, 0x17, 0xe1, 0xa1,
                                                0x29, 0x41, 0x90, 0xd8, 0x3e, 0x42, 0x33, 0xec,
                                                0xc4, 0x90, 0xd3, 0x2d, 0xf1, 0x7a, 0x41, 0x16]);
        
        let actual = super::hash_bytes(&buf_in);
        assert_eq!(expected, actual);
    }

    #[test]
    fn hash_pub_key() {
        let buf_in : Vec<u8> = Vec::new();
        // Generated with hashlib for python
        // h1, h2 = hashlib.sha3_256(), hashlib.new('ripemd160')
        // h2.update(h1.digest())
        // h2.hexdigest()
        let expected = U160::from_big_endian(&[0xb0, 0xa2, 0xc9, 0x10, 0x8b, 0x9c, 0xff, 0x7f,
                                            0x0f, 0x68, 0x6f, 0xef, 0x1d, 0x2e, 0xcb, 0xd5,
                                            0xf1, 0x99, 0x99, 0x72]);
        let actual = super::hash_pub_key(&buf_in);
        assert_eq!(expected, actual);
    }

    #[test]
    fn signing() {
        let data1 = b"This is a message that will be signed; it could instead be a random blob of data...";
        let data2 = b"This is I message that will be signed; it could instead be a random blob of data...";
        // could just use a private key as public key, but want to be sure it works without that.
        let (private_key, public_key) = {
            let rsa = openssl::rsa::Rsa::generate(super::RSA_KEY_SIZE as u32).unwrap();
            let private = PKey::from_rsa(rsa).unwrap();
            let public_pem = private.public_key_to_pem().unwrap();
            let public = PKey::public_key_from_pem(&public_pem).unwrap();
            (private, public)
        };

        let sig = super::sign_bytes(data1, &private_key);
        assert_eq!(sig.len(), super::RSA_KEY_SIZE / 8);
        assert!(super::verify_bytes(data1, &sig, &private_key));
        assert!(super::verify_bytes(data1, &sig, &public_key));
        assert!(!super::verify_bytes(data2, &sig, &private_key));
        assert!(!super::verify_bytes(data2, &sig, &public_key));
    }
}