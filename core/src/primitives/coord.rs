use std::cmp::Ordering;
use bin::*;
use bincode;

/// Square an integer
#[inline(always)]
fn sq(x: i32) -> u64 {
    ((x as i64) * (x as i64)) as u64
}

/// A signed (x, y) coordinate. This can be used as a PlotID.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coord(pub i32, pub i32);

impl Coord {
    pub fn bytes(&self) -> Vec<u8> {
        bincode::serialize(&self, bincode::Bounded(8)).unwrap()
    }
}

impl PartialOrd for Coord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for Coord {
    /// Define the 
    fn cmp(&self, other: &Self) -> Ordering {
        // Calculate the distance from the origin of both points and compare
        let d1 = sq(self.0) + sq(self.1); // should not overflow because we up-size to u64
        let d2 = sq(other.0) + sq(other.1);
        
        if d1 != d2 { d1.cmp(&d2) }
        else if self.0 != other.0 { self.0.cmp(&other.0) }
        else { self.1.cmp(&other.1) }
    }
}

impl AsBin for Coord {
    fn as_bin(&self) -> Bin { bincode::serialize(self, bincode::Bounded(8)).unwrap() }
}


// pub struct BoundingBox {
//     a: Coord,
//     b: Coord
// }