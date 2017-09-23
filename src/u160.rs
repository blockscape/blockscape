use bytes::{ByteOrder, BigEndian, LittleEndian};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use bincode;

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
    /// Print the integer as an aligned hex value
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "0x{:08X}{:08X}{:08X}{:08X}{:08X}",
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
}



#[test]
fn debug() {
    let u = U160([0x0A34DBC3u32, 0x07E6B7BAu32, 0x99821201u32, 0x000271F2u32, 0x95EF424Bu32]);
    assert_eq!(format!("{:?}", u), "0x95EF424B000271F29982120107E6B7BA0A34DBC3");
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
fn serialization() {
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