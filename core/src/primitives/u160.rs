use base16;
use bincode;
use bytes::{ByteOrder, BigEndian, LittleEndian};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;

use serde::ser::{Serialize, Serializer};
use serde::de::*;
use serde::de;

/// A simple 160-bit storage unit that acts sort of like an integer.
/// Note: internally, the lowest significance u32 is in the lowest index,
/// this means that it appears revered when typing a literal array.
#[derive(PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub struct U160([u32; 5]);

/// Defined Zero value for the U160 type.
pub const U160_ZERO: U160 = U160([0u32; 5]);
/// Defined Maximum value for the U160 type.
pub const U160_MAX: U160 = U160([(-1i32) as u32; 5]);

impl fmt::Debug for U160 {
    /// Print the integer as an aligned hex value.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for U160 {
    /// Print the hex value as lowercase with a prefix 0x.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "0x{:08x}{:08x}{:08x}{:08x}{:08x}",
            self.0[4],
            self.0[3],
            self.0[2],
            self.0[1],
            self.0[0]
        )
    }
}

impl Ord for U160 {
    /// Calculate the order between this object and another.
    fn cmp(&self, rhs: &Self) -> Ordering {
        for i in (0..5).rev() {
            let order = self.0[i].cmp(&rhs.0[i]);
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for U160 {
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

impl From<u64> for U160 {
    /// Converts a u64 into a U160, it will be placed in the least-significant position.
    fn from(v: u64) -> U160 {
        U160([(v as u32), ((v >> 32) as u32), 0, 0, 0])
    }
}


impl Hash for U160 {
    /// Calculate the hash value of the little-endian stored bytes using a Hasher.
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut buf = [0u8; 20];
        self.to_little_endian(&mut buf);
        state.write(&buf);
    }
}

impl FromStr for U160 {
    type Err = &'static str;
    
    /// Convert a hex string to a U160 value
    /// # Errors
    /// * If the value is larger than 40 digits (ignoring 0x).
    /// * If the string is empty.
    /// * If any character is invalid.
    fn from_str(v: &str) -> Result<U160, Self::Err> {
        let mut bin = base16::to_bin(v)?;
        let len = bin.len();
        if len > 20 { return Err("Value too large.") }
        
        bin.reverse();
        for _ in len..21 {
            bin.push(0);
        }
        Ok(Self::from_little_endian(&bin))
    }
}

impl U160 {
    /// Checks if the value is zero.
    pub fn is_zero(&self) -> bool {
        *self == U160_ZERO
    }

    /// Reads a U160 arranged in little endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 20 bytes.
    pub fn from_little_endian(slice: &[u8]) -> U160 {
        assert!(slice.len() >= 20);
        let mut v = U160_ZERO;
        for i in 0..5 {
            let s = &slice[(i * 4)..(i * 4 + 4)];
            v.0[i] = LittleEndian::read_u32(s);
        }
        v
    }

    /// Reads a U160 arranged in big endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 20 bytes.
    pub fn from_big_endian(slice: &[u8]) -> U160 {
        assert!(slice.len() >= 20);
        let mut v = U160_ZERO;
        for i in 0..5 {
            let s = &slice[(i * 4)..(i * 4 + 4)];
            v.0[4 - i] = BigEndian::read_u32(s);
        }
        v
    }

    /// Writes a U160 arranged in little endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 20 bytes.
    pub fn to_little_endian(&self, slice: &mut [u8]) {
        assert!(slice.len() >= 20);
        for i in 0..5 {
            let s = &mut slice[(i * 4)..(i * 4 + 4)];
            LittleEndian::write_u32(s, self.0[i]);
        }
    }

    /// Writes a U160 arranged in big endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 20 bytes.
    pub fn to_big_endian(&self, slice: &mut [u8]) {
        assert!(slice.len() >= 20);
        for i in 0..5 {
            let s = &mut slice[(i * 4)..(i * 4 + 4)];
            BigEndian::write_u32(s, self.0[4 - i]);
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        bincode::serialize(&self, bincode::Bounded(20)).unwrap()
     }
}



#[derive(PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct JU160(U160);

impl From<U160> for JU160 {
    fn from(v: U160) -> Self {
        JU160(v)
    }
}

impl Into<U160> for JU160 {
    fn into(self) -> U160 {
        self.0
    }
}

impl Deref for JU160 {
    type Target = U160;

    fn deref(&self) -> &U160 {
        &self.0
    }
}

impl Serialize for JU160 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for JU160 {
    fn deserialize<D>(deserializer: D) -> Result<JU160, D::Error>
        where D: Deserializer<'de>
    {
        struct StrVisitor;

        impl<'de> Visitor<'de> for StrVisitor {
            type Value = JU160;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string")
            }

            fn visit_str<E>(self, value: &str) -> Result<JU160, E>
            where E: de::Error
            {
                value.parse::<U160>()
                    .map(|v| JU160(v))
                    .map_err(Error::custom)
            }
        }

        deserializer.deserialize_string(StrVisitor)
    }
}



#[test]
fn debug() {
    let u = U160([0x0A34DBC3u32, 0x07E6B7BAu32, 0x99821201u32, 0x000271F2u32, 0x95EF424Bu32]);
    assert_eq!(format!("{:?}", u), "0x95ef424b000271f29982120107e6b7ba0a34dbc3");
}

#[test]
fn cmp() {
    let a = U160_MAX;
    let b = U160_ZERO;
    let c = U160([0u32, 0u32, 1u32, 0u32, 0u32]);
    assert_eq!(a.cmp(&b), Ordering::Greater);
    assert_eq!(b.cmp(&a), Ordering::Less);
    assert_eq!(c.cmp(&a), Ordering::Less);
    assert_eq!(c.cmp(&b), Ordering::Greater);
    assert_eq!(c.cmp(&c), Ordering::Equal);
}

#[test]
fn from_u64() {
    let a = 0x95EF424B000271F2u64;
    let b = U160([0x000271F2u32, 0x95EF424Bu32, 0u32, 0u32, 0u32]);
    assert_eq!(U160::from(a), b);
}

#[test]
fn is_zero() {
    let a = U160::from(45352u64);
    let b = U160::from(0u64);
    assert!(U160_ZERO.is_zero());
    assert!(!a.is_zero());
    assert!(b.is_zero());
}

#[test]
fn from_little_endian() {
    let buf: [u8; 20] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                         0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                         0x20, 0x5B, 0xBA, 0x6E];
    let v = U160([0x39FF9897u32, 0xB72B2E17u32, 0xE7514502u32, 0xBEF1FF80u32,  0x6EBA5B20u32]);
    let u = U160::from_little_endian(&buf);
    assert_eq!(u, v);
}

#[test]
fn from_big_endian() {
    let buf: [u8; 20] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                         0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                         0x20, 0x5B, 0xBA, 0x6E];
    let v = U160([0x205BBA6Eu32, 0x80FFF1BEu32, 0x024551E7u32, 0x172E2BB7u32, 0x9798FF39u32]);
    let u = U160::from_big_endian(&buf);
    assert_eq!(u, v);
}

#[test]
fn to_little_endian() {
    let v: [u8; 20] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                         0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                         0x20, 0x5B, 0xBA, 0x6E];
    let mut buf = [0u8; 20];
    let u = U160([0x39FF9897u32, 0xB72B2E17u32, 0xE7514502u32, 0xBEF1FF80u32, 0x6EBA5B20u32]);
    u.to_little_endian(&mut buf);
    assert_eq!(buf, v);
}

#[test]
fn to_big_endian() {
    let v: [u8; 20] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                         0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                         0x20, 0x5B, 0xBA, 0x6E];
    let mut buf =  [0u8; 20];
    let u = U160([0x205BBA6Eu32, 0x80FFF1BEu32, 0x024551E7u32, 0x172E2BB7u32, 0x9798FF39u32]);
    u.to_big_endian(&mut buf);
    assert_eq!(buf, v);
}

#[test]
fn endian_conversions() {
    let start: [u8; 20] = [0xCC, 0xE1, 0xD1, 0xC5, 0x16, 0xF7, 0x1B, 0xBB,
                           0xE3, 0xF1, 0xB9, 0x19, 0x04, 0x39, 0x28, 0xB7,
                           0x51, 0x7B, 0x71, 0xC3];
    let mut buf = [0u8; 20];
    
    let a = U160::from_little_endian(&start);
    a.to_big_endian(&mut buf);

    let b = U160::from_big_endian(&buf);
    assert_eq!(a, b);
    
    b.to_little_endian(&mut buf);
    assert_eq!(start, buf);
}

#[test]
fn from_str() {
    let a = "0xD602B80E32145A890FE49EB2CEE670020E30A580";
    let b = "0x1908199d0ac25cf2ce7942d62dd25bd63c98a66f";
    let c = "2b6f917dc1bab3a3e73c71a6d7a84376577bb144";
    let d = "5C237AD641B42B79B78E1ADF42AEFDFE529E4CA1";
    let e = "2DD8D60B7FD";
    let f = "c7b3c";
    let g = "10000000000000000000000000000000000000000";
    let h = "89347590879087ag";
    assert_eq!(a.parse::<U160>().unwrap(), U160([0x0E30A580, 0xCEE67002, 0x0FE49EB2, 0x32145A89, 0xD602B80E]));
    assert_eq!(b.parse::<U160>().unwrap(), U160([0x3c98a66f, 0x2dd25bd6, 0xce7942d6, 0x0ac25cf2, 0x1908199d]));
    assert_eq!(c.parse::<U160>().unwrap(), U160([0x577bb144, 0xd7a84376, 0xe73c71a6, 0xc1bab3a3, 0x2b6f917d]));
    assert_eq!(d.parse::<U160>().unwrap(), U160([0x529E4CA1, 0x42AEFDFE, 0xB78E1ADF, 0x41B42B79, 0x5C237AD6]));
    assert_eq!(e.parse::<U160>().unwrap(), U160([0x8D60B7FD, 0x000002DD, 0x00000000, 0x00000000, 0x00000000]));
    assert_eq!(f.parse::<U160>().unwrap(), U160([0x000c7b3c, 0x00000000, 0x00000000, 0x00000000, 0x00000000]));
    assert!(g.parse::<U160>().is_err());
    assert!(h.parse::<U160>().is_err());
}

#[test]
fn serialization() {
    use bincode;
    
    let start: [u8; 20] = [0xCC, 0xE1, 0xD1, 0xC5, 0x16, 0xF7, 0x1B, 0xBB,
                           0xE3, 0xF1, 0xB9, 0x19, 0x04, 0x39, 0x28, 0xB7,
                           0x51, 0x7B, 0x71, 0xC3];
    let u = U160::from_little_endian(&start);
    let s = bincode::serialize(&u, bincode::Bounded(20)).unwrap();
    assert_eq!(s.len(), 20);
    let v = bincode::deserialize(&s).unwrap();
    assert_eq!(&s, &start);
    assert_eq!(u, v);
}