use super::*;

/// Stores a large amount of data.
pub struct NANDCard;

impl Describable for NANDCard {
    fn id(&self) -> u8 { 9 }
    fn str_id(&self) -> &'static str { "nand_card" }
    fn object_name(&self) -> &'static str { "NAND Card" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { unimplemented!() }
    fn description(&self) -> &'static str { "Allows for storing a large amount of data with slow access." }
}

impl Worldly for NANDCard {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 { 50_000 }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn p_charge(&self) -> u64 { unimplemented!() }
    fn n_change(&self) -> u64 { unimplemented!() }
    fn max_charge(&self) -> u64 { 5_000_000 }
    fn charge_rate(&self) -> u64 { 40_000 }
    fn passive_cost(&self) -> f32 { 10.0 }
    fn data(&self) -> u64 { unimplemented!() }
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