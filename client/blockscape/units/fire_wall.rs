use super::*;

/// A defense system which blocks enemy units
pub struct FireWall;

impl Describable for FireWall {
    fn id(&self) -> u8 { 11 }
    fn str_id(&self) -> &'static str { "fire_wall" }
    fn object_name(&self) -> &'static str { "Fire Wall" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { None }
    fn description(&self) -> &'static str { "A defense system which blocks enemy units." }
}

impl Worldly for FireWall {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 { 1_000_000 }
    fn decay_rate(&self) -> f32 { 0.02 }
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
    fn energy_cost(&self) -> u64 { 250_000 }
    fn data_cost(&self) -> u64 { 180_000 }
}

impl Structure for FireWall {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (1, 1) }
}