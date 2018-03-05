use super::*;

/// Pathways which aid in bot movement.
pub struct Road;

impl Describable for Road {
    fn id(&self) -> u8 { 1 }
    fn str_id(&self) -> &'static str { "road" }
    fn object_name(&self) -> &'static str { "Road" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { None }
    fn description(&self) -> &'static str { "Pathway which allows bots to travel more rapidly." }
}

impl Worldly for Road {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 { 1000 }
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
    fn energy_cost(&self) -> u64 { 10_000 }
    fn data_cost(&self) -> u64 { 0 }
}

impl Structure for Road {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { false }
    fn xy_len(&self) -> (u32, u32) { (1, 1) }
}