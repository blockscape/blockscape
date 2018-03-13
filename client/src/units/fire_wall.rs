use blockscape_core::primitives::Coord;
use definitions::*;

const MAX_HP: u32 = 1_000_000;

/// A defense system which blocks enemy units
pub struct FireWall {
    location: Coord,
    hp: u32
}

impl Describable for FireWall {
    fn id(&self) -> u8 { 11 }
    fn str_id(&self) -> &'static str { "fire_wall" }
    fn object_name(&self) -> &'static str { "Fire Wall" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { None }
    fn description(&self) -> &'static str { "A defense system which blocks enemy units." }
}

impl Worldly for FireWall {
    fn location(&self) -> Coord { self.location }
    fn hp(&self) -> u32 { self.hp }
    fn max_hp(&self) -> u32 { MAX_HP }
    fn decay_rate(&self) -> f32 { 0.02 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn charge(&self) -> u64 { 0 }
    fn max_charge(&self) -> u64 { 0 }
    fn charge_rate(&self) -> u64 { 0 }
    fn passive_cost(&self) -> f32 { 0.0 }
    fn data(&self) -> u64 { 0 }
    fn max_data(&self) -> u64 { 0 }
    fn transfer_rate(&self) -> u64 { 0 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 250_000 }
    fn data_cost(&self) -> u64 { 180_000 }
}

impl Structure for FireWall {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (1, 1) }
}

impl FireWall {
    pub fn new(location: Coord) -> FireWall {
        FireWall { location, hp: 0 }
    }

    pub fn prebuilt(location: Coord) -> FireWall {
        FireWall { location, hp: MAX_HP }
    }
}