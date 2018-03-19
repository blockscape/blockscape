use blockscape_core::primitives::Coord;
use definitions::*;

const MAX_HP: u32 = 100;

/// Transfers data between buildings.
pub struct DataBus {
    location: Coord,
    hp: u32
}

impl Describable for DataBus {
    fn id(&self) -> u8 { 4 }
    fn str_id(&self) -> &'static str { "data_bus" }
    fn object_name(&self) -> &'static str { "Data Bus" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { None }
    fn description(&self) -> &'static str { "Allows for the transfer of data between structures." }
}

impl Worldly for DataBus {
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
    fn passive_cost(&self) -> f32 { 0.0 } // TODO: Add energy cost
    fn data(&self) -> u64 { 0 }
    fn max_data(&self) -> u64 { 0 }
    fn transfer_rate(&self) -> u64 { 0 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 120_000 }
    fn data_cost(&self) -> u64 { 40_000 }
}

impl Structure for DataBus {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { false }
    fn xy_len(&self) -> (u32, u32) { (1, 1) }
}

impl DataBus {
    pub fn new(location: Coord) -> DataBus {
        DataBus { location, hp: 0 }
    }

    pub fn prebuilt(location: Coord) -> DataBus {
        DataBus { location, hp: MAX_HP }
    }
}