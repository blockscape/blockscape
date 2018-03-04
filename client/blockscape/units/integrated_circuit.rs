use super::*;

/// Data multiplier, takes data and power to make more data.
pub struct IntegratedCircuit;

impl Describable for IntegratedCircuit {
    fn id(&self) -> u8 { 10 }
    fn str_id(&self) -> &'static str { "integrated_circuit" }
    fn object_name(&self) -> &'static str { "Integrated Circuit" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { unimplemented!() }
    fn description(&self) -> &'static str { "Capable of processing data to create more data." }
}

impl Worldly for IntegratedCircuit {
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
    fn charge(&self) -> u64 { unimplemented!() }
    fn max_charge(&self) -> u64 { 50_000_000 }
    fn charge_rate(&self) -> u64 { 200_000 }
    fn passive_cost(&self) -> f32 { 20.0 }
    fn data(&self) -> u64 { unimplemented!() }
    fn max_data(&self) -> u64 { 1_000_000 }
    fn transfer_rate(&self) -> u64 { 100_000 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 30_000_000 }
    fn data_cost(&self) -> u64 { 20_000_000 }
}

impl Structure for IntegratedCircuit {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (5, 5) }
}