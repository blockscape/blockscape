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


/// An area defined by two coordinates. The first coordinate represents the bottom left corner, and
/// the second coordinate represents the top right corner. I.e. the (min x, min y) followed by the
/// (max x, max y)). All functions treat the bounding box as inclusive, that is to say, the borders
/// are considered within the box.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct BoundingBox(Coord, Coord);

impl BoundingBox {
    /// Construct a bounding box from two points. This only requires that the points are opposite
    /// corners of the defined area.
    pub fn new(a: Coord, b: Coord) -> BoundingBox {
        use std::cmp::{min, max};
        BoundingBox(
            Coord(min(a.0, b.0), min(a.1, b.1)),
            Coord(max(a.0, b.0), max(a.1, b.1))
        )
    }

    pub fn area(&self) -> u64 {
        (((self.1).0 - (self.0).0) as u64) * (((self.1).1 - (self.1).0) as u64)
    }

    pub fn top_left(self) -> Coord {
        Coord((self.0).0, (self.1).1)
    }

    pub fn top_right(self) -> Coord {
        self.1
    }

    pub fn bottom_left(self) -> Coord {
        self.0
    }

    pub fn bottom_right(self) -> Coord {
        Coord((self.1).0, (self.0).1)
    }

    pub fn width(self) -> u32 {
        ((self.1).0 - (self.0).0) as u32
    }

    pub fn height(self) -> u32 {
        ((self.1).1 - (self.0).1) as u32
    }

    /// Checks if the point is within this box.
    pub fn contains(self, point: Coord) -> bool {
        point.0 >= (self.0).0 && point.0 <= (self.1).0 &&
        point.1 >= (self.0).1 && point.1 <= (self.1).1
    }

    /// Checks if the other box is completely within this box.
    pub fn contains_box(self, other: BoundingBox) -> bool {
        self.contains(other.0) &&
        self.contains(other.1)
    }

    /// Checks if this box is completely within another box.
    #[inline(always)]
    pub fn is_contained_by(self, other: BoundingBox) -> bool {
        other.contains_box(self)
    }

    /// Checks if any of the area of this box is within the other box.
    pub fn overlaps(self, other: BoundingBox) -> bool {
        self.contains(other.0) ||
        self.contains(other.1)
    }

    /// Calculates the intersection of two boxes and returns the resulting box which is equal to the
    /// overlapping area.
    pub fn intersection(self, other: BoundingBox) -> BoundingBox {
        use std::cmp::{min, max};
        // construct a bounding box with the maximum of the minimum and the minimum of the maximum.
        BoundingBox(
            Coord(max((self.0).0, (other.0).0), max((self.0).1, (other.0).1)),
            Coord(min((self.1).0, (other.1).0), min((self.1).1, (other.1).1))
        )
    }
}