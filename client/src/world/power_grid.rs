use std::collections::{HashSet, VecDeque};
use blockscape_core::primitives::{Coord, Direction};
use definitions::*;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

// A piece of the grid which is described by a starting point, and then a length in the x or y direction.
//struct GridSegment {
//    start: Coord,
//    length: i32  // Positive means in +x direction, negative means in +y direction.
//}

struct StructureNode<'a>(&'a Structure, u8);
impl<'a> Deref for StructureNode<'a> {
    type Target = &'a Structure;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<'a> Hash for StructureNode<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
        self.location().hash(state);
    }
}

impl<'a> PartialEq for StructureNode<'a> {
    fn eq(&self, other: &StructureNode) -> bool {
        self.id() == other.id() && self.location() == other.location()
    }
}

impl<'a> Eq for StructureNode<'a> {}

impl<'a> StructureNode<'a> {
    fn priority(&self) -> u8 { self.1 }
}



/// Concept of a power or data grid. It is the over-arching result of many components that can be
/// reduced to a list of producers and consumers.
pub struct Grid<'a> {
    /// The structures which compose the gird.
    segments: HashSet<Coord>,
    /// The producer/consumer buildings, and their priority with 0 being the highest priority.
    buildings: HashSet<StructureNode<'a>>
}

impl<'a> Grid<'a> {
    /// Create a new grid given the first segment in it.
    pub fn new(segment: Coord) -> Grid<'a> {
        Grid {segments: hashset!(segment), buildings: hashset!() }
    }

    /// If a new segment was added, true is returned. It may not add a segment if it was already
    /// part of the network, or if it is not adjacent to the network.
    pub fn add_segment(&mut self, segment: Coord) -> bool {
        if self.is_adj(segment) {
            self.segments.insert(segment)
        } else { false }
    }

    /// Remove one or more segments from the grid, this may result in multiple disconnected grids.
    pub fn remove_segments(mut self, segments: HashSet<Coord>) -> Vec<Grid<'a>> {
        self.segments = self.segments.difference(&segments).cloned().collect();
        if self.segments.is_empty() { return vec![]; }

        // We just removed the segment, so we need to perform BFS and determine if it is one
        // network or two.
        let mut queue = VecDeque::new();
        let mut grids = Vec::new();

        // May have separate disconnected grids
        while !self.segments.is_empty() {
            let mut new_grid = Grid { segments: hashset!(), buildings: hashset!() };
            // arbitrary start point
            queue.push_back(*self.segments.iter().next().unwrap());

            // explore all connected segments
            while !queue.is_empty() {
                let s = queue.pop_front().unwrap();

                // explore adjacent places and add to queue.
                for &dir in Direction::cardinal_iterator() {
                    let n = s.move_in_dir(dir, 1);
                    if self.segments.contains(&n) {
                        self.segments.remove(&n);
                        queue.push_back(n);
                    }
                }

                // add this segment to the new grid.
                new_grid.segments.insert(s);
            }

            // we have fully explored this sub-grid
            grids.push(new_grid);
        }

        // attempt to add the buildings to each of the grids. A single building may be connected
        // to more than one of the grids.
        for grid in grids.iter_mut() {
            for building in self.buildings.iter() {
                grid.add_building(building.0, building.1);
            }
        } grids
    }

    /// Adds a building to the producer/consumer pool if it is connected physically adjacent to the
    /// grid segments. Returns true if the building was added.
    pub fn add_building(&mut self, structure: &'a Structure, priority: u8) -> bool {
        if self.is_structure_adj(structure) {
            self.buildings.insert(StructureNode(structure, priority))
        } else { false }
    }

    /// Check if a structure is adjacent to the grid, if so, then it is capable of being connected
    /// to the power grid.
    pub fn is_structure_adj(&self, structure: &Structure) -> bool {
        let base = structure.location();
        let (xl, yl) = {
            let (x, y) = structure.xy_len();
            (x as i32, y as i32)
        };

        // Just check outside border around it (not including the exterior corners)
        for dx in 0..xl {
            if self.is_grid(base.0 + dx, base.1 - 1) ||  // bottom border
                self.is_grid(base.0 + dx, base.1 + yl) {  // top border
                return true;
            }
        }

        for dy in 0..yl {
            if self.is_grid(base.0 - 1, base.1 + dy) ||  // left border
                self.is_grid(base.0 + xl, base.1 + dy) {  // right border
                return true;
            }fn hello() {

}
        }

        false
    }

    /// Check if a given point is next to the grid.
    pub fn is_adj(&self, point: Coord) -> bool {
        self.segments.contains(&point) ||
        self.is_grid(point.0 - 1, point.1) ||
        self.is_grid(point.0 + 1, point.1) ||
        self.is_grid(point.0, point.1 - 1) ||
        self.is_grid(point.0, point.1 + 1)
    }

    /// Check if a building is currently connected to the grid.
    pub fn is_connected(&self, s: &'a Structure) -> bool {
        self.buildings.contains(&StructureNode(s, 0))
    }

    /// checks if a given tile is part of the grid.
    #[inline(always)]
    fn is_grid(&self, x: i32, y: i32) -> bool {
        self.segments.contains(&Coord(x, y))
    }
}



#[test]
fn is_adj() {
    let mut grid = Grid::new(Coord(0, 0));
    // Cardinal directions are adj
    assert!(grid.is_adj(Coord(0, -1)));
    assert!(grid.is_adj(Coord(0, 1)));
    assert!(grid.is_adj(Coord(-1, 0)));
    assert!(grid.is_adj(Coord(1, 0)));
    assert!(grid.is_adj(Coord(0, 0)));

    // Diagonals are not adj
    assert!(!grid.is_adj(Coord(-1, -1)));
    assert!(!grid.is_adj(Coord(1, 1)));
    assert!(!grid.is_adj(Coord(-1, 1)));
    assert!(!grid.is_adj(Coord(1, -1)));
}

#[test]
fn add_segment() {
    let mut grid = Grid::new(Coord(0, 0));
    assert!(!grid.add_segment(Coord(0, 2)));
    assert!(grid.add_segment(Coord(0, 1)));
    assert!(grid.add_segment(Coord(0, 2)));
    assert!(grid.add_segment(Coord(0, 3)));
    assert!(grid.add_segment(Coord(1, 2)));
    assert!(grid.add_segment(Coord(2, 2)));
    assert!(!grid.add_segment(Coord(3, 3)));
}

#[test]
fn remove_segments() {
    let grid = Grid { segments: hashset!(
        Coord(0, 0), Coord(0, 1), Coord(0, 2), Coord(1, 0), Coord(2, 0), Coord(3, 0),
        Coord(-1, 0), Coord( -2, 0), Coord(0, -1), Coord(0, -2)
    ), buildings: hashset!() };

    let res = grid.remove_segments(hashset!(Coord(3, 0)));
    assert_eq!(res.len(), 1);

    let grid = res.into_iter().next().unwrap();
    assert!(!grid.segments.contains(&Coord(3, 0)));
    let res = grid.remove_segments(hashset!(Coord(0, 0)));
    assert_eq!(res.len(), 4);
}

//#[test]
//fn add_building() {
//    use units::Beacon;
//    let grid = Grid::new(Coord(0, 0));
//    let building = Beacon;
//}