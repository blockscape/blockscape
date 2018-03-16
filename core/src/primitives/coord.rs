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

    /// The area included in the box. Note: because the boundaries are part of the box, the area
    /// includes the edges.
    #[inline]
    pub fn area(self) -> u64 {
        self.width() as u64 * self.height() as u64
    }

    /// The top left corner of the box.
    #[inline]
    pub fn top_left(self) -> Coord {
        Coord((self.0).0, (self.1).1)
    }

    /// The top right corner of the box.
    #[inline(always)]
    pub fn top_right(self) -> Coord {
        self.1
    }

    /// The bottom left corner of the box.
    #[inline(always)]
    pub fn bottom_left(self) -> Coord {
        self.0
    }

    /// The bottom right corner of the box.
    #[inline]
    pub fn bottom_right(self) -> Coord {
        Coord((self.1).0, (self.0).1)
    }

    /// The width of the box.
    #[inline]
    pub fn width(self) -> u32 {
        ((self.1).0 - (self.0).0) as u32 + 1
    }

    /// The height of the box.
    #[inline]
    pub fn height(self) -> u32 {
        ((self.1).1 - (self.0).1) as u32 + 1
    }

    /// Checks if the point is within this box.
    #[inline]
    pub fn contains(self, point: Coord) -> bool {
        point.0 >= (self.0).0 && point.0 <= (self.1).0 &&
        point.1 >= (self.0).1 && point.1 <= (self.1).1
    }

    /// Checks if the other box is completely within this box.
    #[inline]
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
        self.contains(other.top_left()) ||
        self.contains(other.top_right()) ||
        self.contains(other.bottom_left()) ||
        self.contains(other.bottom_right()) ||
        other.contains(self.top_left()) ||
        other.contains(self.top_right()) ||
        other.contains(self.bottom_left()) ||
        other.contains(self.bottom_right())
    }

    /// Calculates the intersection of two boxes and returns the resulting box which is equal to the
    /// overlapping area. This requires that the two bounding boxes overlap.
    pub fn intersect(self, other: BoundingBox) -> Option<BoundingBox> {
        if !self.overlaps(other) { return None; }

        use std::cmp::{min, max};
        // construct a bounding box with the maximum of the minimum and the minimum of the maximum.
        Some( BoundingBox(
            Coord(max((self.0).0, (other.0).0), max((self.0).1, (other.0).1)),
            Coord(min((self.1).0, (other.1).0), min((self.1).1, (other.1).1))
        ))
    }
}



#[test]
fn sq_dist() {
    assert_eq!(Coord(0, 0).sq_dist(Coord(5, 5)), 50);
    assert_eq!(Coord(-1, 0).sq_dist(Coord(5, 5)), 61);
    assert_eq!(Coord(-5, 8).sq_dist(Coord(7, -1)), 225);
}

#[test]
fn bounding_box_new() {
    assert_eq!(
        BoundingBox::new(Coord(0, 0), Coord(5, 5)),
        BoundingBox(Coord(0, 0), Coord(5, 5))
    );

    assert_eq!(
        BoundingBox::new(Coord(-3, 0), Coord(5, 0)),
        BoundingBox(Coord(-3, 0), Coord(5, 0))
    );

    assert_eq!(
        BoundingBox::new(Coord(8, -3), Coord(-1, 3)),
        BoundingBox(Coord(-1, -3), Coord(8, 3))
    );
}

#[test]
fn area() {
    assert_eq!(BoundingBox(Coord(0, 0), Coord(4, 4)).area(), 25);
    assert_eq!(BoundingBox(Coord(0, 0), Coord(0, 0)).area(), 1);
    assert_eq!(BoundingBox(Coord(0, 0), Coord(4, 16)).area(), 85);
    assert_eq!(BoundingBox(Coord(-1, 3), Coord(1, 3)).area(), 3);
}

#[test]
fn top_left() {
    assert_eq!(BoundingBox(Coord(0, 0), Coord(3, 3)).top_left(), Coord(0, 3));
    assert_eq!(BoundingBox(Coord(-2, 4), Coord(2, 10)).top_left(), Coord(-2, 10));
}

#[test]
fn top_right() {
    assert_eq!(BoundingBox(Coord(0, 0), Coord(3, 3)).top_right(), Coord(3, 3));
    assert_eq!(BoundingBox(Coord(-2, 4), Coord(2, 10)).top_right(), Coord(2, 10));
}

#[test]
fn bottom_left() {
    assert_eq!(BoundingBox(Coord(0, 0), Coord(3, 3)).bottom_left(), Coord(0, 0));
    assert_eq!(BoundingBox(Coord(-2, 4), Coord(2, 10)).bottom_left(), Coord(-2, 4));
}

#[test]
fn bottom_right() {
    assert_eq!(BoundingBox(Coord(0, 0), Coord(3, 3)).bottom_right(), Coord(3, 0));
    assert_eq!(BoundingBox(Coord(-2, 4), Coord(2, 10)).bottom_right(), Coord(2, 4));
}

#[test]
fn width() {
    assert_eq!(BoundingBox(Coord(0, 0), Coord(5, 3)).width(), 6);
    assert_eq!(BoundingBox(Coord(-2, 5), Coord(5, 7)).width(), 8);
}

#[test]
fn height() {
    assert_eq!(BoundingBox(Coord(0, 0), Coord(5, 3)).height(), 4);
    assert_eq!(BoundingBox(Coord(-2, 5), Coord(5, 7)).height(), 3);
}

#[test]
fn contains() {
    let bb = BoundingBox(Coord(-2, 0), Coord(3, 3));
    assert!(bb.contains(Coord(0, 0)));
    assert!(bb.contains(Coord(3, 3)));
    assert!(bb.contains(Coord(-2, 0)));
    assert!(bb.contains(Coord(-1, 2)));
    assert!(!bb.contains(Coord(-3, -1)));
    assert!(!bb.contains(Coord(4, 0)));
}

#[test]
fn contains_box() {
    let a = BoundingBox(Coord(-2, 0), Coord(3, 3));
    let b = BoundingBox(Coord(-1, 0), Coord(2, 1));
    let c = BoundingBox(Coord(0, 0), Coord(10, 10));
    assert!(a.contains_box(b));
    assert!(!b.contains_box(a));
    assert!(a.contains_box(a));
    assert!(b.contains_box(b));
    assert!(!a.contains_box(c));
    assert!(!c.contains_box(a));
}

#[test]
fn overlaps() {
    let a = BoundingBox(Coord(-2, 0), Coord(3, 3));
    let b = BoundingBox(Coord(0, 0), Coord(10, 10));
    let c = BoundingBox(Coord(5, 0), Coord(7, 1));
    assert!(a.overlaps(b));
    assert!(b.overlaps(a));
    assert!(a.overlaps(a));
    assert!(b.overlaps(b));
    assert!(!a.overlaps(c));
    assert!(!c.overlaps(a));
    assert!(b.overlaps(c));
    assert!(c.overlaps(b));
    assert!(a.overlaps(BoundingBox(Coord(0, -2), Coord(5, 1)))); // top left and bottom right corner overlap
    assert!(a.overlaps(BoundingBox(Coord(-3, -1), Coord(4, 4)))); // second box is completely surrounding A
}

#[test]
fn intersect() {
    let a = BoundingBox(Coord(-2, 0), Coord(3, 3));
    let b = BoundingBox(Coord(0, 0), Coord(10, 10));
    let c = BoundingBox(Coord(0, -2), Coord(5, 1));
    assert_eq!(a.intersect(b).unwrap(), BoundingBox(Coord(0, 0), Coord(3, 3)));
    assert_eq!(b.intersect(a).unwrap(), BoundingBox(Coord(0, 0), Coord(3, 3)));
    assert_eq!(a.intersect(c).unwrap(), BoundingBox(Coord(0, 0), Coord(3, 1)));
    assert_eq!(a.intersect(BoundingBox(Coord(-3, -1), Coord(4, 4))).unwrap(), a);
    assert_eq!(c.intersect(c).unwrap(), c);
    assert_eq!(a.intersect(BoundingBox(Coord(5, 0), Coord(7, 1))), None);
}