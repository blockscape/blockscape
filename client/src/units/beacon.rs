use blockscape_core::primitives::Coord;
use definitions::*;

/// Allows querying information from other plots and to send/receive active messages.
pub struct Beacon;

impl Describable for Beacon {
    fn id(&self) -> u8 { 13 }
    fn str_id(&self) -> &'static str { "beacon" }
    fn object_name(&self) -> &'static str { "Beacon" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { unimplemented!() }
    fn description(&self) -> &'static str { "Capable of sending and receiving messages between plots." }
}

impl Worldly for Beacon {
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
    fn charge(&self) -> u64 { unimplemented!() }
    fn max_charge(&self) -> u64 { 20_000_000 }
    fn charge_rate(&self) -> u64 { 100_000 }
    fn passive_cost(&self) -> f32 { 10.0 }
    fn data(&self) -> u64 { unimplemented!() }
    fn max_data(&self) -> u64 { 5_000_000 }
    fn transfer_rate(&self) -> u64 { 20_000 }
    fn passive_data(&self) -> f32 { 0.1 }
    fn energy_cost(&self) -> u64 { 20_000_000 }
    fn data_cost(&self) -> u64 { 5_000_000 }
}

impl Structure for Beacon {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (2, 2) }
}