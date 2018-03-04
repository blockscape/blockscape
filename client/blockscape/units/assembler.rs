use super::*;

/// Constructs and repairs robots.
pub struct Assembler;

impl Describable for Assembler {
    fn id(&self) -> u8 { 12 }
    fn str_id(&self) -> &'static str { "assembler" }
    fn object_name(&self) -> &'static str { "Assembler" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { unimplemented!() }
    fn description(&self) -> &'static str { "Capable of constructing bots." }
}

impl Worldly for Assembler {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 { 20_000 }
    fn decay_rate(&self) -> f32 { 0.006 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn p_charge(&self) -> u64 { unimplemented!() }
    fn n_change(&self) -> u64 { unimplemented!() }
    fn max_charge(&self) -> u64 { 50_000_000 }
    fn charge_rate(&self) -> u64 { 200_000 }
    fn passive_cost(&self) -> f32 { 5.0 }
    fn data(&self) -> u64 { unimplemented!() }
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