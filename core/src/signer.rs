use openssl;
use openssl::{sign, hash};
use openssl::pkey::PKey;

use serde::Serialize;

use bincode;

/// The size of any new RSA Keys; other sizes should still be supported.
pub const RSA_KEY_SIZE: usize = 2048;

/// Sign some bytes with a private key.
pub fn sign_bytes(bytes: &[u8], private_key: &PKey) -> Vec<u8> {
    let mut signer = sign::Signer::new(hash::MessageDigest::sha256(), &private_key).unwrap();
    signer.update(bytes).unwrap();
    signer.finish().unwrap()
}

/// Verify the bytes have not been tampered with given a signature and public key.
pub fn verify_bytes(bytes: &[u8], signature: &[u8], public_key: &PKey) -> bool {
    let mut verifier = sign::Verifier::new(hash::MessageDigest::sha256(), public_key).unwrap();
    verifier.update(bytes).unwrap();
    verifier.finish(&signature).unwrap()
}

/// Sign an object with a private key.
pub fn sign_obj<S: Serialize>(obj: &S, private_key: &PKey) -> Vec<u8> {
    let encoded: Vec<u8> = bincode::serialize(&obj, bincode::Infinite).unwrap();
    sign_bytes(&encoded, private_key)
}

/// Verify the object has not been tampered with given the signature and public key.
pub fn verify_obj<S: Serialize>(obj: &S, signature: &[u8], public_key: &PKey) -> bool {
    let encoded: Vec<u8> = bincode::serialize(&obj, bincode::Infinite).unwrap();
    verify_bytes(&encoded, signature, public_key)
}

pub fn generate_private_key() -> PKey {
    let rsa = openssl::rsa::Rsa::generate(RSA_KEY_SIZE as u32).unwrap();
    PKey::from_rsa(rsa).unwrap()
}