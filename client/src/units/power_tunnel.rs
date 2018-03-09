use blockscape_core::primitives::Coord;
use definitions::*;

/// Allows transfer of energy between plots.
pub struct PowerTunnel;

impl Describable for PowerTunnel {
    fn id(&self) -> u8 { 3 }
    fn str_id(&self) -> &'static str { "power_tunnel" }
    fn object_name(&self) -> &'static str { "Power Tunnel" }
    fn instance_name(&self) -> Option<String> { None }
    fn status(&self) -> Option<String> { None }
    fn description(&self) -> &'static str { "Allows for the transfer of energy between plots." }
}

impl Worldly for PowerTunnel {
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
    fn charge(&self) -> u64 { 0 }
    fn max_charge(&self) -> u64 { 0 }
    fn charge_rate(&self) -> u64 { 0 }
    fn passive_cost(&self) -> f32 { 0.0 }
    fn data(&self) -> u64 { 0 }
    fn max_data(&self) -> u64 { 0 }
    fn transfer_rate(&self) -> u64 { 0 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 8_000_000 }
    fn data_cost(&self) -> u64 { 900_000 }
}

impl Structure for PowerTunnel {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (3, 3) }
}