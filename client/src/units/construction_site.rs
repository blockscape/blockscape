use definitions::*;
use blockscape_core::primitives::Coord;

pub struct ConstructionSite {
    obj: Box<Worldly>,
    charge: u64,
    data: u64,
    hp: u32
}

impl Describable for ConstructionSite {
    fn id(&self) -> u8 { 255 }
    fn str_id(&self) -> &'static str { "construction_site" }
    fn object_name(&self) -> &'static str { "Construction Site" }
    fn instance_name(&self) -> Option<&str> { None }
    fn status(&self) -> Option<&str> { Some(super::STATUS_UNBUILT) }
    fn description(&self) -> &'static str { "The construction site a worldly object." }
}

impl Worldly for ConstructionSite {
    fn location(&self) -> Coord { self.obj.location() }
    fn hp(&self) -> u32 { self.hp }
    fn max_hp(&self) -> u32 { 10 }
    fn decay_rate(&self) -> f32 { 0.0001 }
    fn sp(&self) -> u32 { 0 }
    fn max_sp(&self) -> u32 { 0 }
    fn sp_regen_rate(&self) -> u32 { 0 }
    fn sp_regen_cost(&self) -> f32 { 0.0 }
    fn charge(&self) -> u64 { self.charge }
    fn max_charge(&self) -> u64 { self.obj.energy_cost() }
    fn charge_rate(&self) -> u64 { 0 }
    fn passive_cost(&self) -> f32 { 0.0}
    fn data(&self) -> u64 { self.data }
    fn max_data(&self) -> u64 { self.obj.data_cost() }
    fn transfer_rate(&self) -> u64 { 0 }
    fn passive_data(&self) -> f32 { 0.0 }
    fn energy_cost(&self) -> u64 { 0 }
    fn data_cost(&self) -> u64 { 0 }
}

impl ConstructionSite {
    pub fn new(structure: Box<Worldly>) -> ConstructionSite {
        ConstructionSite { obj: structure, charge: 0, data: 0, hp: 10 }
    }
}