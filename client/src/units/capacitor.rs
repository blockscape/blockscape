use blockscape_core::primitives::Coord;
use definitions::*;

const MAX_HP: u32 = 5_000;

/// Stores a small amount of charge.
pub struct Capacitor {
    status: &'static str,
    location: Coord,
    charge: u64,
    hp: u32
}

impl Describable for Capacitor {
    fn id(&self) -> u8 { 6 }
    fn str_id(&self) -> &'static str { "capacitor" }
    fn object_name(&self) -> &'static str { "Capacitor" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { Some(self.status) }
    fn description(&self) -> &'static str { "Allows for storing a small amount of electric charge." }
}

impl Worldly for Capacitor {
    fn location(&self) -> Coord { self.location }
    fn hp(&self) -> u32 { self.hp }
    fn max_hp(&self) -> u32 { MAX_HP }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn charge(&self) -> u64 { self.charge }
    fn max_charge(&self) -> u64 { 1_000_000 }
    fn charge_rate(&self) -> u64 { 400_000 }
    fn passive_cost(&self) -> f32 { 5.0 }
    fn data(&self) -> u64 { 0 }
    fn max_data(&self) -> u64 { 0 }
    fn transfer_rate(&self) -> u64 { 0 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 1_000_000 }
    fn data_cost(&self) -> u64 { 80_000 }
}

impl Structure for Capacitor {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (3, 3) }
}

impl Capacitor {
    pub fn new(location: Coord) -> Capacitor {
        Capacitor { status: super::STATUS_UNBUILT, location, charge: 0, hp: 0 }
    }

    pub fn prebuilt(location: Coord) -> Capacitor {
        Capacitor { status: super::STATUS_IDLE, location, charge: 0, hp: MAX_HP }
    }
}