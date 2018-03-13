use definitions::*;
use blockscape_core::primitives::Coord;

pub struct ConstructionSite;

impl Describable for ConstructionSite {
    fn id(&self) -> u8 {
        unimplemented!()
    }

    fn str_id(&self) -> &'static str {
        unimplemented!()
    }

    fn object_name(&self) -> &'static str {
        unimplemented!()
    }

    fn instance_name(&self) -> Option<&str> {
        unimplemented!()
    }

    fn status(&self) -> Option<&str> {
        unimplemented!()
    }

    fn description(&self) -> &'static str {
        unimplemented!()
    }
}

impl Worldly for ConstructionSite {
    fn location(&self) -> Coord {
        unimplemented!()
    }

    fn hp(&self) -> u32 {
        unimplemented!()
    }

    fn max_hp(&self) -> u32 {
        unimplemented!()
    }

    fn decay_rate(&self) -> f32 {
        unimplemented!()
    }

    fn sp(&self) -> u32 {
        unimplemented!()
    }

    fn max_sp(&self) -> u32 {
        unimplemented!()
    }

    fn sp_regen_rate(&self) -> u32 {
        unimplemented!()
    }

    fn sp_regen_cost(&self) -> f32 {
        unimplemented!()
    }

    fn charge(&self) -> u64 {
        unimplemented!()
    }

    fn max_charge(&self) -> u64 {
        unimplemented!()
    }

    fn charge_rate(&self) -> u64 {
        unimplemented!()
    }

    fn passive_cost(&self) -> f32 {
        unimplemented!()
    }

    fn data(&self) -> u64 {
        unimplemented!()
    }

    fn max_data(&self) -> u64 {
        unimplemented!()
    }

    fn transfer_rate(&self) -> u64 {
        unimplemented!()
    }

    fn passive_data(&self) -> f32 {
        unimplemented!()
    }

    fn energy_cost(&self) -> u64 {
        unimplemented!()
    }

    fn data_cost(&self) -> u64 {
        unimplemented!()
    }
}