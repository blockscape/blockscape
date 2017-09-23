use bytes::{ByteOrder, BigEndian, LittleEndian};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use bincode;

/// A simple 256-bit storage unit that acts sort of like an integer.
/// Note: internally, the lowest significance u64 is in the lowest index,
/// this means that it appears revered when typing a literal array.
#[derive(PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub struct U256([u64; 4]);

/// Defined Zero value for the U256 type.
pub const U256_ZERO: U256 = U256([0u64; 4]);
/// Defined Maximum value for the U256 type.
pub const U256_MAX: U256 = U256([(-1i64) as u64; 4]);

impl fmt::Debug for U256 {
    /// Print the integer as an aligned hex value
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "0x{:016X}{:016X}{:016X}{:016X}",
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
        U256([v, 0, 0, 0])
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
        for i in 0..4 {
            let s = &slice[(i * 8)..(i * 8 + 8)];
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
        for i in 0..4 {
            let s = &slice[(i * 8)..(i * 8 + 8)];
            v.0[3 - i] = BigEndian::read_u64(s);
        }
        v
    }

    /// Writes a U256 arranged in little endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 32 bytes.
    pub fn to_little_endian(&self, slice: &mut [u8]) {
        assert!(slice.len() >= 32);
        for i in 0..4 {
            let s = &mut slice[(i * 8)..(i * 8 + 8)];
            LittleEndian::write_u64(s, self.0[i]);
        }
    }

    /// Writes a U256 arranged in big endian format.
    /// # Panics
    /// Panics if the length of `slice` is less than 32 bytes.
    pub fn to_big_endian(&self, slice: &mut [u8]) {
        assert!(slice.len() >= 32);
        for i in 0..4 {
            let s = &mut slice[(i * 8)..(i * 8 + 8)];
            BigEndian::write_u64(s, self.0[3-i]);
        }
    }
}



#[test]
fn debug() {
    let u = U256([0x0A34DBC36A8EBA78u64, 0x07E6B7BA2207330Au64, 0x95EF424B99821201u64, 0x000271F22FE33752u64]);
    assert_eq!(format!("{:?}", u), "0x000271F22FE3375295EF424B9982120107E6B7BA2207330A0A34DBC36A8EBA78");
}

#[test]
fn cmp() {
    let a = U256_MAX;
    let b = U256_ZERO;
    let c = U256([0u64, 0u64, 1u64, 0u64]);
    assert_eq!(a.cmp(&b), Ordering::Greater);
    assert_eq!(b.cmp(&a), Ordering::Less);
    assert_eq!(c.cmp(&a), Ordering::Less);
    assert_eq!(c.cmp(&b), Ordering::Greater);
    assert_eq!(c.cmp(&c), Ordering::Equal);
}

#[test]
fn from_u64() {
    let a = 986543u64;
    let b = U256([a, 0u64, 0u64, 0u64]);
    assert_eq!(U256::from(a), b);
}

#[test]
fn is_zero() {
    let a = U256::from(45352u64);
    let b = U256::from(0u64);
    assert!(U256_ZERO.is_zero());
    assert!(!a.is_zero());
    assert!(b.is_zero());
}

#[test]
fn from_little_endian() {
    /* # Calculated with:
     * a = [random.randrange(2**8) for _ in range(32)]
     * b = [0, 0, 0, 0]
     * for i in range(4):
     *     for j in range(8):
     *         b[i] += a[i*8+j] << j*8
     */
    let buf: [u8; 32] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                         0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                         0x20, 0x5B, 0xBA, 0x6E, 0xF7, 0x68, 0x9F, 0x36,
                         0x1C, 0x8C, 0x24, 0x46, 0xC9, 0x6D, 0xC4, 0xC6];
    let v = U256([0xB72B2E1739FF9897u64, 0xBEF1FF80E7514502u64, 0x369F68F76EBA5B20u64, 0xC6C46DC946248C1Cu64]);
    let u = U256::from_little_endian(&buf);
    assert_eq!(u, v);
}

#[test]
fn from_big_endian() {
    /* # Calculated with:
     * a = [random.randrange(2**8) for _ in range(32)]
     * b = [0, 0, 0, 0]
     * for i in reversed(range(4)):
     *     for j in range(8):
     *         b[3-i] += a[i*8+(7-j)] << j*8
     */
    let buf: [u8; 32] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                         0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                         0x20, 0x5B, 0xBA, 0x6E, 0xF7, 0x68, 0x9F, 0x36,
                         0x1C, 0x8C, 0x24, 0x46, 0xC9, 0x6D, 0xC4, 0xC6];
    let v = U256([0x1C8C2446C96DC4C6u64, 0x205BBA6EF7689F36u64, 0x024551E780FFF1BEu64, 0x9798FF39172E2BB7u64]);
    let u = U256::from_big_endian(&buf);
    assert_eq!(u, v);
}

#[test]
fn to_little_endian() {
    let v: [u8; 32] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                       0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                       0x20, 0x5B, 0xBA, 0x6E, 0xF7, 0x68, 0x9F, 0x36,
                       0x1C, 0x8C, 0x24, 0x46, 0xC9, 0x6D, 0xC4, 0xC6];
    let mut buf =  [0u8; 32];
    let u = U256([0xB72B2E1739FF9897u64, 0xBEF1FF80E7514502u64, 0x369F68F76EBA5B20u64, 0xC6C46DC946248C1Cu64]);
    u.to_little_endian(&mut buf);
    assert_eq!(buf, v);
}

#[test]
fn to_big_endian() {
    let v: [u8; 32] = [0x97, 0x98, 0xFF, 0x39, 0x17, 0x2E, 0x2B, 0xB7,
                       0x02, 0x45, 0x51, 0xE7, 0x80, 0xFF, 0xF1, 0xBE,
                       0x20, 0x5B, 0xBA, 0x6E, 0xF7, 0x68, 0x9F, 0x36,
                       0x1C, 0x8C, 0x24, 0x46, 0xC9, 0x6D, 0xC4, 0xC6];
    let mut buf =  [0u8; 32];
    let u = U256([0x1C8C2446C96DC4C6u64, 0x205BBA6EF7689F36u64, 0x024551E780FFF1BEu64, 0x9798FF39172E2BB7u64]);
    u.to_big_endian(&mut buf);
    assert_eq!(buf, v);
}

#[test]
fn endian_conversions() {
    let start: [u8; 32] = [0xCC, 0xE1, 0xD1, 0xC5, 0x16, 0xF7, 0x1B, 0xBB,
                           0xE3, 0xF1, 0xB9, 0x19, 0x04, 0x39, 0x28, 0xB7,
                           0x51, 0x7B, 0x71, 0xC3, 0x86, 0xF0, 0xCF, 0x2A,
                           0x34, 0xFA, 0x9C, 0x18, 0x04, 0x6B, 0xF6, 0x36];
    let mut buf = [0u8; 32];
    
    let a = U256::from_little_endian(&start);
    a.to_big_endian(&mut buf);

    let b = U256::from_big_endian(&buf);
    assert_eq!(a, b);
    
    b.to_little_endian(&mut buf);
    assert_eq!(start, buf);
}

#[test]
fn serialization() {
    let start: [u8; 32] = [0xCC, 0xE1, 0xD1, 0xC5, 0x16, 0xF7, 0x1B, 0xBB,
                           0xE3, 0xF1, 0xB9, 0x19, 0x04, 0x39, 0x28, 0xB7,
                           0x51, 0x7B, 0x71, 0xC3, 0x86, 0xF0, 0xCF, 0x2A,
                           0x34, 0xFA, 0x9C, 0x18, 0x04, 0x6B, 0xF6, 0x36];
    let u = U256::from_little_endian(&start);
    let s = bincode::serialize(&u, bincode::Bounded(32)).unwrap();
    assert_eq!(s.len(), 32);
    let v = bincode::deserialize(&s).unwrap();
    assert_eq!(&s, &start);
    assert_eq!(u, v);
}