use std::collections::HashMap;
use bit_vec::BitVec;
use blockscape_core::primitives::Coord;
use super::*;

const PLOT_SIZE: usize = 256;

const ROAD_TRAVEL_COST: usize = 10;
const NON_ROAD_TRAVEL_COST: usize = ROAD_TRAVEL_COST * 3;
const DIAGONAL_ROAD_TRAVEL_COST: usize = (ROAD_TRAVEL_COST as f64 * std::f64::consts::SQRT_2) as usize;
const DIAGONAL_NON_ROAD_TRAVEL_COST: usize = (NON_ROAD_TRAVEL_COST as f64 * std::f64::consts::SQRT_2) as usize;

pub struct Plot<'a> {
    height_map: BitVec,
    roads: BitVec,

    structures: Vec<Box<Structure>>, //TODO: VecDeq?
    agents: Vec<Box<Agent<'a>>>,
    mobs: Vec<Box<Mobile<'a>>>
}

impl<'a> Plot<'a> {
    pub fn generate() -> Plot<'a> {
        Plot {
            height_map: BitVec::from_elem(PLOT_SIZE * PLOT_SIZE, false),
            // structures: hashmap!{ coord_to_index(Coord(PLOT_SIZE/2, PLOT_SIZE/2)) =>  }, //TODO: once we have a CPU structure definition, put it in the center of the plot.
            structures: Vec::new(),
            agents: Vec::new(),
            mobs: Vec::new(),

            roads: BitVec::from_elem(PLOT_SIZE * PLOT_SIZE, false)
        }
    }

    /// Returns the neighboring coordinates to a given coordinate and the cost to get there. Will
    /// not include coordinates which are not traversable (i.e. height map is 1).
    pub fn neighboring_paths(loc: Coord) -> Vec<(Coord, usize)> {
        unimplemented!()
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