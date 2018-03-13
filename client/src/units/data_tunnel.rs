use blockscape_core::primitives::Coord;
use definitions::*;

const MAX_HP: u32 = 5_000;

/// Allows transfer of data between plots.
pub struct DataTunnel {
    status: &'static str,
    location: Coord,
    hp: u32
}

impl Describable for DataTunnel {
    fn id(&self) -> u8 { 5 }
    fn str_id(&self) -> &'static str { "data_tunnel" }
    fn object_name(&self) -> &'static str { "Data Tunnel" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { Some(self.status) }
    fn description(&self) -> &'static str { "Allows for the transfer of data between plots." }
}

impl Worldly for DataTunnel {
    fn location(&self) -> Coord { self.location }
    fn hp(&self) -> u32 { self.hp }
    fn max_hp(&self) -> u32 { MAX_HP }
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
    fn energy_cost(&self) -> u64 { 12_000_000 }
    fn data_cost(&self) -> u64 { 7_000_000 }
}

impl Structure for DataTunnel {
    fn category(&self) -> &'static str { "infrastructure" }
    fn blocking(&self) -> bool { true }
    fn xy_len(&self) -> (u32, u32) { (3, 3) }
}

impl DataTunnel {
    pub fn new(location: Coord) -> DataTunnel {
        DataTunnel { status: super::STATUS_UNBUILT, location, hp: 0 }
    }

    pub fn prebuilt(location: Coord) -> DataTunnel {
        DataTunnel { status: super::STATUS_IDLE, location, hp: MAX_HP }
    }
}