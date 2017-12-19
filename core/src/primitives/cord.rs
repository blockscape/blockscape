use std::cmp::Ordering;
use bincode;

/// Square an integer
#[inline(always)]
fn sq(x: i32) -> u64 {
    ((x as i64) * (x as i64)) as u64
}

/// A signed (x, y) coordinate. This can be used as a PlotID.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cord {
    pub x: i32,
    pub y: i32
}

impl Cord {
    pub fn bytes(&self) -> Vec<u8> {
        bincode::serialize(&self, bincode::Bounded(8)).unwrap()
    }
}

impl PartialOrd for Cord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for Cord {
    /// Define the 
    fn cmp(&self, other: &Self) -> Ordering {
        // Calculate the distance from the origin of both points and compare
        let d1 = sq(self.x) + sq(self.y); // should not overflow because we up-size to u64
        let d2 = sq(other.x) + sq(other.y);
        
        if d1 != d2 { d1.cmp(&d2) }
        else if self.x != other.x { self.x.cmp(&other.x) }
        else { self.y.cmp(&other.y) }
    }
}


// pub struct BoundingBox {
//     a: Cord,
//     b: Cord
// }