use super::*;

/// Stores a large amount of charge.
pub struct Battery;

impl Describable for Battery {
    fn id(&self) -> u8 { 7 }
    fn str_id(&self) -> &'static str { "battery" }
    fn object_name(&self) -> &'static str { "Battery" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { unimplemented!() }
    fn description(&self) -> &'static str { "Allows for storing a large amount of electric charge." }
}

impl Worldly for Battery {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 { 6000 }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn p_charge(&self) -> u64 { unimplemented!() }
    fn n_change(&self) -> u64 { unimplemented!() }
    fn max_charge(&self) -> u64 { 80_000_000 }
    fn charge_rate(&self) -> u64 { 100_000 }
    fn passive_cost(&self) -> f32 { 8.0 }
    fn data(&self) -> u64 { 0 }
    fn max_data(&self) -> u64 { 0 }
    fn transfer_rate(&self) -> u64 { 0 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 6_000_000 }
    fn data_cost(&self) -> u64 { 600_000 }
}

impl Structure for Battery {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (4, 4) }
}