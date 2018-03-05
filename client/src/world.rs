use std::collections::HashMap;
use bit_vec::BitVec;
use blockscape_core::primitives::Coord;
use super::*;

const PLOT_SIZE: usize = 256;

pub struct Plot<'a> {
    height_map: BitVec,
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
            mobs: Vec::new()
        }
    }

    fn coord_to_index(coord: Coord) -> usize {
        let x = if coord.0 >= 0 { coord.0 } else { PLOT_SIZE as i32 + coord.0 };
        let y = if coord.1 >= 0 { coord.1 } else { PLOT_SIZE as i32 + coord.1 };

        assert!(0 <= x && 0 <= y);
        assert!(x < PLOT_SIZE as i32 && y < PLOT_SIZE as i32);

        (PLOT_SIZE - y as usize) * PLOT_SIZE + x as usize
    }
}