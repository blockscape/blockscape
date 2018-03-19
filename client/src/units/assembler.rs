use blockscape_core::primitives::Coord;
use definitions::*;

const MAX_HP: u32 = 20_000;

/// Constructs and repairs robots.
pub struct Assembler {
    status: &'static str,
    location: Coord,
    charge: u64,
    data: u64,
    hp: u32
}

impl Describable for Assembler {
    fn id(&self) -> u8 { 12 }
    fn str_id(&self) -> &'static str { "assembler" }
    fn object_name(&self) -> &'static str { "Assembler" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { Some(self.status) }
    fn description(&self) -> &'static str { "Capable of constructing bots." }
}

impl Worldly for Assembler {
    fn location(&self) -> Coord { self.location }
    fn hp(&self) -> u32 { self.hp }
    fn max_hp(&self) -> u32 { 0 }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn charge(&self) -> u64 { self.charge }
    fn max_charge(&self) -> u64 { 50_000_000 }
    fn charge_rate(&self) -> u64 { 200_000 }
    fn passive_cost(&self) -> f32 { 5.0 }
    fn data(&self) -> u64 { self.data }
    fn max_data(&self) -> u64 { 25_000_000 }
    fn transfer_rate(&self) -> u64 { 100_000 }
    fn passive_data(&self) -> f32 { 0.1 }
    fn energy_cost(&self) -> u64 { 20_000_000 }
    fn data_cost(&self) -> u64 { 5_000_000 }
}

impl Structure for Assembler {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (5, 8) }
}

impl Assembler {
    pub fn new(location: Coord) -> Assembler {
        Assembler { status: super::STATUS_UNBUILT, location, charge: 0, data: 0, hp: 0 }
    }

    pub fn prebuilt(location: Coord) -> Assembler {
        Assembler { status: super::STATUS_IDLE, location, charge: 0, data: 0, hp: MAX_HP }
    }
}