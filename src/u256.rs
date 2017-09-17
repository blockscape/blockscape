use bytes::{ByteOrder, BigEndian, LittleEndian};
use crypto::digest::Digest;
use crypto::sha3::{Sha3, Sha3Mode};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A simple 256-bit storage unit that acts sort of like an integer.
#[derive(PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub struct U256([u64; 4]);

/// Defined Zero value for the U256 type.
const U256_ZERO: U256 = U256([0u64; 4]);
/// Defined Maximum value for the U256 type.
const U256_MAX: U256 = U256([(-1i64) as u64; 4]);

impl fmt::Debug for U256 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "0x{:X}{:X}{:X}{:X}",
            self.0[3],
            self.0[2],
            self.0[1],
            self.0[0]
        )
    }
}

impl Ord for U256 {
    /// Calculate the order between this object and another.
    fn cmp(&self, rhs: &Self) -> Ordering {
        for i in (0..4).rev() {
            let order = self.0[i].cmp(&rhs.0[i]);
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for U256 {
    /// Calculate the order between this object and another.
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }

    /// Calculate self < rhs
    fn lt(&self, rhs: &Self) -> bool {
        match self.cmp(rhs) {
            Ordering::Less => true,
            _ => false,
        }
    }

    /// Calculate self <= rhs
    fn le(&self, rhs: &Self) -> bool {
        match self.cmp(rhs) {
            Ordering::Less => true,
            Ordering::Equal => true,
            _ => false,
        }
    }

    /// Calculate self > rhs
    fn gt(&self, rhs: &Self) -> bool {
        !(self <= rhs)
    }
    /// Calculate self >= rhs
    fn ge(&self, rhs: &Self) -> bool {
        !(self < rhs)
    }
}

impl From<u64> for U256 {
    /// Converts a u64 into a U256, it will be placed in the least-significant position.
    fn from(v: u64) -> U256 {
        U256([0, 0, 0, v])
    }
}

impl Hash for U256 {
    /// Calculate the hash value of the little-endian stored bytes using a Hasher.
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut buf = [0u8; 32];
        self.to_little_endian(&mut buf);
        state.write(&buf);
    }
}

impl U256 {
    /// Returns the maximum of two U256 values.
    pub fn max(self, rhs: Self) -> Self {
        match self.cmp(&rhs) {
            Ordering::Less => rhs,
            Ordering::Equal => self,
            Ordering::Greater => self,
        }
    }

    /// Returns the minimum of two U256 values.
    pub fn min(self, rhs: Self) -> Self {
        match self.cmp(&rhs) {
            Ordering::Less => self,
            Ordering::Equal => self,
            Ordering::Greater => rhs,
        }
    }

    /// Checks if the value is zero.
    pub fn is_zero(&self) -> bool {
        *self == U256_ZERO
    }

    /// Reads a U256 arranged in little endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 32 bytes.
    pub fn from_little_endian(slice: &[u8]) -> U256 {
        assert!(slice.len() >= 32);
        let mut v = U256_ZERO;
        for i in (0..4) {
            let s = &slice[(i * 64)..(i * 64 + 64)];
            v.0[i] = LittleEndian::read_u64(s);
        }
        v
    }

    /// Reads a U256 arranged in big endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 32 bytes.
    pub fn from_big_endian(slice: &[u8]) -> U256 {
        assert!(slice.len() >= 32);
        let mut v = U256_ZERO;
        for i in (0..4) {
            let s = &slice[(i * 64)..(i * 64 + 64)];
            v.0[3 - i] = BigEndian::read_u64(s);
        }
        v
    }

    /// Writes a U256 arranged in little endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 32 bytes.
    pub fn to_little_endian(&self, slice: &mut [u8]) {
        assert!(slice.len() >= 32);
        for i in (0..4) {
            let s = &mut slice[(i * 64)..(i * 64 + 64)];
            LittleEndian::write_u64(s, self.0[i]);
        }
    }

    /// Writes a U256 arranged in big endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 32 bytes.
    pub fn to_big_endian(&self, slice: &mut [u8]) {
        assert!(slice.len() >= 32);
        for i in (0..4) {
            let s = &mut slice[(i * 64)..(i * 64 + 64)];
            BigEndian::write_u64(s, self.0[3 - i]);
        }
    }

    /// Support for rust-crypto. Take a `Digest` object and feed it the little-endian bytes
    /// of this data.
    pub fn crypto_digest<D: Digest>(&self, state: &mut D) {
        let mut buf = [0u8; 32];
        self.to_little_endian(&mut buf);
        state.input(&buf);
    }

    /// Calculate the sha3-256 value for this object using the little-endian bytes.
    /// Note: This is less efficient when calculting many hashes than using `crypto_digest`.
    pub fn sha3_256(&self) -> U256 {
        let mut buf = [0u8; 32];
        let mut hasher = Sha3::new(Sha3Mode::Sha3_256);
        self.crypto_digest::<Sha3>(&mut hasher);
        hasher.result(&mut buf);
        U256::from_little_endian(&buf)
    }
}