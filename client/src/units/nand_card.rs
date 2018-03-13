use blockscape_core::primitives::Coord;
use definitions::*;

const MAX_HP: u32 = 50_000;

/// Stores a large amount of data.
pub struct NANDCard {
    status: &'static str,
    location: Coord,
    charge: u64,
    data: u64,
    hp: u32
}

impl Describable for NANDCard {
    fn id(&self) -> u8 { 9 }
    fn str_id(&self) -> &'static str { "nand_card" }
    fn object_name(&self) -> &'static str { "NAND Card" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { Some(self.status) }
    fn description(&self) -> &'static str { "Allows for storing a large amount of data with slow access." }
}

impl Worldly for NANDCard {
    fn location(&self) -> Coord { self.location }
    fn hp(&self) -> u32 { self.hp }
    fn max_hp(&self) -> u32 { MAX_HP }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn charge(&self) -> u64 { self.charge }
    fn max_charge(&self) -> u64 { 5_000_000 }
    fn charge_rate(&self) -> u64 { 40_000 }
    fn passive_cost(&self) -> f32 { 10.0 }
    fn data(&self) -> u64 { self.data }
    fn max_data(&self) -> u64 { 15_000_000 }
    fn transfer_rate(&self) -> u64 { 80_000 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 9_000_000 }
    fn data_cost(&self) -> u64 { 2_000_000 }
}

impl Structure for NANDCard {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (6, 5) }
}

impl NANDCard {
    pub fn new(location: Coord) -> NANDCard {
        NANDCard { status: super::STATUS_UNBUILT, location, charge: 0, data: 0, hp: 0 }
    }

    pub fn prebuilt(location: Coord) -> NANDCard {
        NANDCard { status: super::STATUS_IDLE, location, charge: 0, data: 0, hp: MAX_HP }
    }
}