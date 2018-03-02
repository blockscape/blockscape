use super::*;

/// Stores a small amount of data (with electric cost).
pub struct RAMCard;

impl Describable for RAMCard {
    fn id(&self) -> u8 { 8 }
    fn str_id(&self) -> &'static str { "ram_card" }
    fn object_name(&self) -> &'static str { "RAM Card" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { unimplemented!() }
    fn description(&self) -> &'static str { "Allows for storing a small amount of data with rapid access." }
}

impl Worldly for RAMCard {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 { 10_000 }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn p_charge(&self) -> u64 { unimplemented!() }
    fn n_change(&self) -> u64 { unimplemented!() }
    fn max_charge(&self) -> u64 { 5_000_000 }
    fn charge_rate(&self) -> u64 { 100_000 }
    fn passive_cost(&self) -> f32 { 10.0 }
    fn data(&self) -> u64 { unimplemented!() }
    fn max_data(&self) -> u64 { 2_000_000 }
    fn transfer_rate(&self) -> u64 { 500_000 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 12_000_000 }
    fn data_cost(&self) -> u64 { 4_000_000 }
}

impl Structure for RAMCard {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (5, 2) }
}