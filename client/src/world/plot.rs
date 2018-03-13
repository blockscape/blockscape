use definitions::*;
use blockscape_core::primitives::{Coord, Direction};
use bit_vec::BitVec;
use super::power_grid::Grid;
use std::rc::Rc;

use std;


const PLOT_SIZE: usize = 256;

const ROAD_TRAVEL_COST: usize = 10;
const NON_ROAD_TRAVEL_COST: usize = ROAD_TRAVEL_COST * 3;
const DIAGONAL_ROAD_TRAVEL_COST: usize = (ROAD_TRAVEL_COST as f64 * std::f64::consts::SQRT_2) as usize;
const DIAGONAL_NON_ROAD_TRAVEL_COST: usize = (NON_ROAD_TRAVEL_COST as f64 * std::f64::consts::SQRT_2) as usize;

pub struct Plot<'a> {
    height_map: BitVec,
    roads: BitVec,
    power_grids: Vec<Grid>,
    data_grids: Vec<Grid>,

    structures: Vec<Rc<Structure>>, //TODO: VecDeq?
    agents: Vec<Rc<Agent<'a>>>,
    mobs: Vec<Rc<Mobile<'a>>>
}

impl<'a> Plot<'a> {
    pub fn generate() -> Plot<'a> {
        Plot {
            height_map: BitVec::from_elem(PLOT_SIZE * PLOT_SIZE, false),
            // structures: hashmap!{ coord_to_index(Coord(PLOT_SIZE/2, PLOT_SIZE/2)) =>  }, //TODO: once we have a CPU structure definition, put it in the center of the plot.
            roads: BitVec::from_elem(PLOT_SIZE * PLOT_SIZE, false),
            power_grids: vec![],
            data_grids: vec![],

            structures: vec![],
            agents: vec![],
            mobs: vec![]
        }
    }

    /// Returns the neighboring coordinates to a given coordinate and the cost to get there. Will
    /// not include coordinates which are not traversable (i.e. height map is 1).
    pub fn neighboring_paths(&self, loc: Coord) -> Vec<(Coord, usize)> {
        let mut neighbors = Vec::new();
        for dir in Direction::iterator() {
            let n_loc = loc.move_in_dir(*dir, 1);
            if n_loc.0 >= PLOT_SIZE as i32 || n_loc.0 < 0 || n_loc.1 >= PLOT_SIZE as i32 || n_loc.1 < 0 {
                // we have gone past the edge, don't add it as an option
                // TODO: handle inter-plot pathfinding
                continue;
            }

            let i = Self::coord_to_index(n_loc);

            // If we cannot move onto this tile, don't add it as an option
            if !self.height_map[i] { continue; }

            let cost = if self.roads[i] {
                if dir.is_cardinal() { ROAD_TRAVEL_COST }
                    else { DIAGONAL_ROAD_TRAVEL_COST }
            } else {
                if dir.is_cardinal() { NON_ROAD_TRAVEL_COST }
                    else { DIAGONAL_NON_ROAD_TRAVEL_COST }
            };

            neighbors.push((n_loc, cost))
        }

        neighbors
    }

    /// Convert a coordinate into an index for a 1D array representation of the 2D data.
    /// Treats negative numbers as an index from the far end, e.g. `(-1, -100)` would be
    /// `(PLOT_SIZE - 1, PLOT_SIZE - 100)`.
    fn coord_to_index(coord: Coord) -> usize {
        let x = if coord.0 >= 0 { coord.0 } else { PLOT_SIZE as i32 + coord.0 };
        let y = if coord.1 >= 0 { coord.1 } else { PLOT_SIZE as i32 + coord.1 };

        assert!(0 <= x && 0 <= y);
        assert!(x < PLOT_SIZE as i32 && y < PLOT_SIZE as i32);

        y as usize * PLOT_SIZE + x as usize
    }
}