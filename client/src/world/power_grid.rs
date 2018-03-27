use std::collections::{HashSet, VecDeque};
use blockscape_core::primitives::{Coord, Direction};
use definitions::*;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::fmt::{Debug}; use std::fmt;

// A piece of the grid which is described by a starting point, and then a length in the x or y direction.
//struct GridSegment {
//    start: Coord,
//    length: i32  // Positive means in +x direction, negative means in +y direction.
//}

struct StructureNode(Rc<Structure>, u8);
impl Deref for StructureNode {
    type Target = Rc<Structure>;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Hash for StructureNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
        self.location().hash(state);
    }
}

impl PartialEq for StructureNode {
    fn eq(&self, other: &StructureNode) -> bool {
        self.id() == other.id() && self.location() == other.location()
    }
}

impl Eq for StructureNode {}

impl Debug for StructureNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "StructureNode({}, {:?}, {})", self.object_name(), self.location(), self.1)
    }
}

impl StructureNode {
    fn priority(&self) -> u8 { self.1 }
}



/// Concept of a power or data grid. It is the over-arching result of many components that can be
/// reduced to a list of producers and consumers.
#[derive(Debug)]
pub struct Grid {
    /// The structures which compose the gird.
    segments: HashSet<Coord>,
    /// The producer/consumer buildings, and their priority with 0 being the highest priority.
    buildings: HashSet<StructureNode>
}

impl Grid {
    /// Create a new grid given the first segment in it.
    pub fn new(segment: Coord) -> Grid {
        Grid {segments: hashset!(segment), buildings: hashset!() }
    }

    /// If a new segment was added, true is returned. It will not be added if it was already
    /// part of the network. This requires that the segment is adjcent to the network.
    pub fn add_segment(&mut self, segment: Coord) -> bool {
        debug_assert!(self.is_adj(segment));
        self.segments.insert(segment)
    }

    /// Remove one or more segments from the grid, this may result in multiple disconnected grids.
    pub fn remove_segments(mut self, segments: &HashSet<Coord>) -> Vec<Grid> {
        self.segments = self.segments.difference(segments).cloned().collect();
        if self.segments.is_empty() { return vec![]; }

        // We just removed the segment, so we need to perform BFS and determine if it is one
        // network or two.
        let mut queue = VecDeque::new();
        let mut grids = Vec::new();

        // May have separate disconnected grids
        let mut count = 0;
        while !self.segments.is_empty() {
            let mut new_grid = Grid { segments: hashset!(), buildings: hashset!() };
            // arbitrary start point
            queue.push_back(*self.segments.iter().next().unwrap());
            self.segments.remove(&queue[0]);

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
                grid.add_building(Rc::clone(&building.0), building.1);
            }
        }

        grids
    }

    /// Will merge another grid into this grid. The precondition for calling this is that the two
    /// grids are in fact connected as this will simply assume they are.
    fn merge(&mut self, other: Grid) {
        for s in other.segments.into_iter() { self.segments.insert(s); }
        for b in other.buildings.into_iter() { self.buildings.insert(b); }
    }

    /// Adds a building to the producer/consumer pool if it is connected physically adjacent to the
    /// grid segments. Returns true if the building was added. Expects the building to be adjacent
    /// to the network.
    pub fn add_building(&mut self, structure: Rc<Structure>, priority: u8) -> bool {
        debug_assert!(self.is_structure_adj(&*structure));
        self.buildings.insert(StructureNode(structure, priority))
    }

    /// Remove a building from the grid.
    pub fn remove_building(&mut self, structure: Rc<Structure>) -> bool {
        self.buildings.remove(&StructureNode(structure, 0))
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

    /// Check if a given point is in the grid.
    pub fn contains_segment(&self, point: Coord) -> bool {
        self.segments.contains(&point)
    }

    /// Check if a building is currently connected to the grid.
    pub fn is_connected(&self, s: Rc<Structure>) -> bool {
        self.buildings.contains(&StructureNode(s, 0))
    }

    /// checks if a given tile is part of the grid.
    #[inline(always)]
    fn is_grid(&self, x: i32, y: i32) -> bool {
        self.segments.contains(&Coord(x, y))
    }
}



/// A system of independent grids which can be merged and separated.
pub struct GridNet(VecDeque<Grid>);

impl Deref for GridNet {
    type Target = VecDeque<Grid>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GridNet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl GridNet {
    pub fn new() -> GridNet {
        GridNet(VecDeque::new())
    }

    pub fn add_segment(&mut self, segment: Coord) {
        // Attempt to add the segment to all grids and merge those which are duplicates
        let mut master: Option<Grid> = None;
        for _ in 0..self.len() {
            let mut grid = self.pop_front().unwrap();
            if grid.is_adj(segment) {
                if master.is_some() { // the segment spans networks, merge them
                    master.as_mut().unwrap().merge(grid);
                } else { // found the first one to add to
                    grid.add_segment(segment);
                    master = Some(grid);
                }
            } else {
                // re-add it because it is not relevant
                self.push_back(grid)
            }
        }

        if let Some(master) = master {
            self.push_back(master)
        } else {
            self.push_back(Grid::new(segment));
        }
    }

    /// Remove one or more segments from the grid network.
    pub fn remove_segments(&mut self, segments: &HashSet<Coord>) {
        // Attempt to remove the segments for all the grids, and add the resulting ones back to the
        // master list.
        for x in 0..self.len() {
            let grid = self.pop_front().unwrap();
            let grids = grid.remove_segments(segments);
            for g in grids.into_iter() { self.push_back(g); }
        }
    }

    /// Add a new producer/consumer to the grid.
    pub fn add_building(&mut self, structure: &Rc<Structure>, priority: u8) -> bool {
        let mut added = false;
        for grid in self.iter_mut() {
            if grid.is_structure_adj(structure.as_ref()) {
                added = true;
                grid.add_building(Rc::clone(structure), priority);
            }
        } added
    }

    pub fn remove_building(&mut self, structure: &Rc<Structure>) -> bool {
        let mut removed = false;
        for grid in self.iter_mut() {
            if grid.remove_building(Rc::clone(structure)) { removed = true; }
        } removed
    }
}



#[test]
fn grid_is_adj() {
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
fn grid_add_segment() {
    let mut grid = Grid::new(Coord(0, 0));
    assert!(grid.add_segment(Coord(0, 1)));
    assert!(grid.add_segment(Coord(0, 2)));
    assert!(grid.add_segment(Coord(0, 3)));
    assert!(grid.add_segment(Coord(1, 2)));
    assert!(grid.add_segment(Coord(2, 2)));
    assert!(!grid.add_segment(Coord(2, 2)));
    assert!(!grid.add_segment(Coord(0, 1)));
}

#[test]
fn grid_remove_segments() {
    let grid = Grid { segments: hashset!(
        Coord(0, 0), Coord(0, 1), Coord(0, 2), Coord(1, 0), Coord(2, 0), Coord(3, 0),
        Coord(-1, 0), Coord( -2, 0), Coord(0, -1), Coord(0, -2)
    ), buildings: hashset!() };

    let res = grid.remove_segments(&hashset!(Coord(3, 0)));
    assert_eq!(res.len(), 1);

    let grid = res.into_iter().next().unwrap();
    assert!(!grid.segments.contains(&Coord(3, 0)));
    let res = grid.remove_segments(&hashset!(Coord(0, 0)));
    assert_eq!(res.len(), 4);
}

#[test]
fn gridnet_add_segment() {
    let mut gnet = GridNet::new();
    gnet.add_segment(Coord(-1, 0));
    assert_eq!(gnet.len(), 1);

    gnet.add_segment(Coord(-2, 0));
    assert_eq!(gnet.len(), 1);
    assert_eq!(gnet[0].segments.len(), 2);

    gnet.add_segment(Coord(1, 0));
    assert_eq!(gnet.len(), 2);

    gnet.add_segment(Coord(0, 1));
    assert_eq!(gnet.len(), 3);

    gnet.add_segment(Coord(0, -1));
    assert_eq!(gnet.len(), 4);

    gnet.add_segment(Coord(0, 0));
    assert_eq!(gnet.len(), 1);
    assert_eq!(gnet[0].segments.len(), 6);
}

#[test]
fn gridnet_remove_segments() {
    let mut gnet = GridNet(vec![
        Grid { buildings: hashset!(), segments: hashset!(Coord(0, 0), Coord(-1, 0), Coord(0, -1), Coord(1, 0), Coord(0, 1)) },
        Grid { buildings: hashset!(), segments: hashset!(Coord(10, 10), Coord(11, 10), Coord(12, 10)) },
        Grid { buildings: hashset!(), segments: hashset!(Coord(15, 15), Coord(15, 16)) }
    ].into_iter().collect());

    assert_eq!(gnet.len(), 3);
    gnet.remove_segments(&hashset!(Coord(0, 0), Coord(10, 10), Coord(12, 10)));
    assert_eq!(gnet.len(), 6);

    gnet.remove_segments(&hashset!(Coord(11,10)));
    assert_eq!(gnet.len(), 5);

    gnet.remove_segments(&hashset!(Coord(15, 15), Coord(15, 16)));
    assert_eq!(gnet.len(), 4);
}