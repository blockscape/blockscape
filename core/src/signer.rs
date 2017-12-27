use bincode;
use openssl;
use openssl::{sign, hash};
use openssl::pkey::PKey;
use serde::Serialize;
use bin::Bin;

/// The size of any new RSA Keys; other sizes should still be supported.
pub const RSA_KEY_SIZE: usize = 2048;

/// Sign some bytes with a private key.
pub fn sign_bytes(bytes: &[u8], private_key: &PKey) -> Vec<u8> {
    let mut signer = sign::Signer::new(hash::MessageDigest::sha256(), &private_key).unwrap();
    signer.update(bytes).unwrap();
    signer.sign_to_vec().unwrap()
}

/// Sign some binary data with a private key.
#[inline]
pub fn sign_bin(bin: &Bin, private_key: &PKey) -> Bin {
    sign_bytes(&*bin, private_key).into()
}

/// Verify the bytes have not been tampered with given a signature and public key.
pub fn verify_bytes(bytes: &[u8], signature: &[u8], public_key: &PKey) -> bool {
    let mut verifier = sign::Verifier::new(hash::MessageDigest::sha256(), public_key).unwrap();
    verifier.update(bytes).unwrap();
    verifier.verify(&signature).unwrap()
}

/// Verify the binary data has not been tampered with given it's signature and public key.
#[inline]
pub fn verify_bin(bin: &Bin, signature: &Bin, public_key: &PKey) -> bool {
    verify_bytes(&*bin, &*signature, public_key)
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

#[test]
fn signing() {
    let data1 = b"This is a message that will be signed; it could instead be a random blob of data...";
    let data2 = b"This is I message that will be signed; it could instead be a random blob of data...";
    // could just use a private key as public key, but want to be sure it works without that.
    let (private_key, public_key) = {
        let rsa = openssl::rsa::Rsa::generate(RSA_KEY_SIZE as u32).unwrap();
        let private = PKey::from_rsa(rsa).unwrap();
        let public_pem = private.public_key_to_pem().unwrap();
        let public = PKey::public_key_from_pem(&public_pem).unwrap();
        (private, public)
    };

    let sig = sign_bytes(data1, &private_key);
    assert_eq!(sig.len(), RSA_KEY_SIZE / 8);
    assert!(verify_bytes(data1, &sig, &private_key));
    assert!(verify_bytes(data1, &sig, &public_key));
    assert!(!verify_bytes(data2, &sig, &private_key));
    assert!(!verify_bytes(data2, &sig, &public_key));
}