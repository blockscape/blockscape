extern crate bincode;
extern crate bytes;
extern crate crypto;
extern crate dns_lookup;
extern crate openssl;
extern crate serde_json;
extern crate serde;
extern crate rand;
extern crate time as timelib;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

pub mod block;
pub mod txn;
pub mod hash;
pub mod mutation;
pub mod network;
pub mod signer;
pub mod u256;
pub mod u160;
pub mod time;
pub mod env;

use serde::Serialize;
use u256::U256;
use u160::U160;

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