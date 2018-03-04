use super::*;

/// Stores a small amount of charge.
pub struct Capacitor;

impl Describable for Capacitor {
    fn id(&self) -> u8 { 6 }
    fn str_id(&self) -> &'static str { "capacitor" }
    fn object_name(&self) -> &'static str { "Capacitor" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { unimplemented!() }
    fn description(&self) -> &'static str { "Allows for storing a small amount of electric charge." }
}

impl Worldly for Capacitor {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 { 5000 }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn p_charge(&self) -> u64 { unimplemented!() }
    fn n_change(&self) -> u64 { unimplemented!() }
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