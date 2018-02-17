use std::collections::HashMap;
use bit_vec::BitVec;
use blockscape_core::primitives::Coord;
use super::*;

const PLOT_SIZE: usize = 256;

struct Plot<'a> {
    height_map: BitVec,
    structures: HashMap<Coord, Box<Structure>>,
    agents: HashMap<Coord, Box<Agent<'a>>>,
    mobs: HashMap<Coord, Box<Mobile<'a>>>
}