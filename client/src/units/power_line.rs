use blockscape_core::primitives::Coord;
use definitions::*;

const MAX_HP: u32 = 100;

/// Transfers +/- charge between buildings.
pub struct PowerLine {
    location: Coord,
    hp: u32
}

impl Describable for PowerLine {
    fn id(&self) -> u8 { 2 }
    fn str_id(&self) -> &'static str { "power_line" }
    fn object_name(&self) -> &'static str { "Power Line" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { None }
    fn description(&self) -> &'static str { "Allows for the transfer of energy between structures." }
}

impl Worldly for PowerLine {
    fn location(&self) -> Coord { self.location }
    fn hp(&self) -> u32 { self.hp }
    fn max_hp(&self) -> u32 { MAX_HP }
    fn decay_rate(&self) -> f32 { 0.006 }
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
    fn energy_cost(&self) -> u64 { 80_000 }
    fn data_cost(&self) -> u64 { 6_000 }
}

impl Structure for PowerLine {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { false }
    fn xy_len(&self) -> (u32, u32) { (1, 1) }
}

impl PowerLine {
    pub fn new(location: Coord) -> PowerLine {
        PowerLine { location, hp: 0 }
    }

    pub fn prebuilt(location: Coord) -> PowerLine {
        PowerLine {location, hp: MAX_HP }
    }
}