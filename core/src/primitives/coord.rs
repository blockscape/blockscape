use std::cmp::Ordering;
use bin::*;
use bincode;
use super::Direction;

macro_rules! abs_diff {
    ($a:expr, $b:expr) => {{
        if $a > $b { $a - $b}
        else { $b - $a }
    }}
}

/// Square an integer
#[inline(always)]
fn sq(x: i32) -> u64 {
    ((x as i64) * (x as i64)) as u64
}

/// A signed (x, y) coordinate. This can be used as a PlotID.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coord(pub i32, pub i32);

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

impl Coord {

    /// Returns the squared distance between two points.
    #[inline]
    pub fn sq_dist(self, other: Coord) -> u64 {
        let dx = abs_diff!(self.0, other.0) as u64;
        let dy = abs_diff!(self.1, other.1) as u64;
        dx*dx + dy*dy
    }

    /// Returns the euclidean distance between two points.
    #[inline]
    pub fn dist(self, other: Coord) -> f64 {
        (self.sq_dist(other) as f64).sqrt()
    }

    /// Get the coordinate a given number of "steps" in a specified direction.
    pub fn move_in_dir(self, dir: Direction, steps: usize) -> Coord {
        let (dx, dy) = dir.dx_dy();
        Coord(
            self.0 + dx as i32 * steps as i32,
            self.1 + dy as i32 * steps as i32
        )
    }
}


// pub struct BoundingBox {
//     a: Coord,
//     b: Coord
// }