use std::ops::{Deref, DerefMut};
use std::collections::HashMap;

/// The individual statistics which may be modified. This way we can simply include a list of the
/// ones which we want to change and by how much.
#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum BaseStat {
    MaxHP, MaxSP, SPRegen, SPRegenCost, MaxCharge, ChargeRate, PassiveCost, MaxData, TransferRate,
    PassiveData, EnergyCost, DataCost, SiphonRate, SiphonLoss, SiphonCost, MiningRate, MiningLoss,
    MiningCost, BuildRate, BuildCost, RepairRate, RepairCost, ReclaimRate, ReclaimLoss, ReclaimCost,
    MeleeDamage, MeleeRolls, MeleeAccuracy, MeleeCost, RangedDamage, RangedRolls, RangedAccuracy,
    RangedCost, RangedData, RotationTimeout, RotationCost, MoveTimeout, MoveCost
}


/// The different modules which may be inserted into agents. Each of them have different effects on
/// the base stats.
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum Module {
    Move, Capacitor, Memory, Siphon, IOPort, Booster, Overclock, Construction, Repair, Reclaim,
    Melee, Ranged, Aim, Armor, Shield, CPU
}

impl Module {
    /// Get the percentage changes from the base and also the flat increase and decrease amounts.
    fn get_effects(&self) -> Vec<(BaseStat, f32, i64)> {
        use self::BaseStat::*;
        use self::Module::*;
        match *self {
            Move => vec![
                (PassiveCost, 0.00, 5),
                (MoveTimeout, -0.25, 0),
                (MoveCost, 0.20, 30),
                (RotationTimeout, -0.40, 0),
                (RotationCost, 0.10, 10),
                (RangedAccuracy, -0.01, 0),
                (EnergyCost, 0.04, 2_000_000),
                (DataCost, 0.02, 100_000)],
            Capacitor => vec![
                (MaxCharge, 0.05, 1_000_000),
                (MoveTimeout, 0.00, 2),
                (EnergyCost, 0.04, 500_000),
                (DataCost, 0.02, 24_000)],
            Memory => vec![
                (MaxData, 0.00, 1_000_000),
                (MoveTimeout, 0.00, 2),
                (PassiveCost, 0.00, 10),
                (EnergyCost, 0.04, 800_000),
                (DataCost, 0.02, 60_000)],
            Siphon => vec![
                (SiphonRate, 0.00, 10_000),
                (SiphonLoss, 0.05, 2),
                (SiphonCost, 0.00, 10),
                (ChargeRate, 0.00, 10_000),
                (EnergyCost, 0.04, 1_000_000),
                (DataCost, 0.02, 50_000)],
            IOPort => vec![
                (MiningRate, 0.00, 10_000),
                (MiningLoss, 0.05, 100),
                (MiningCost, 0.00, 60),
                (TransferRate, 0.00, 10_000),
                (EnergyCost, 0.04, 1_500_000),
                (DataCost, 0.02, 80_000)],
            Booster => vec![
                (PassiveCost, 0.00, 2),
                (ChargeRate, 0.00, 100_000),
                (EnergyCost, 0.01, 300_000),
                (DataCost, 0.005, 10_000)],
            Overclock => vec![
                (TransferRate, 0.00, 100_000),
                (PassiveCost, 0.00, 4),
                (EnergyCost, 0.01, 700_000),
                (DataCost, 0.005, 96_000)],
            Construction => vec![
                (ReclaimRate, 0.00, 20),
                (ReclaimLoss, 0.02, 1),
                (ReclaimCost, 0.01, 5),
                (BuildRate, 0.00, 100),
                (BuildCost, 0.05, 10),
                (RepairRate, 0.01, 20),
                (RepairCost, 0.01, 5),
                (EnergyCost, 0.04, 5_000_000),
                (DataCost, 0.02, 600_000)],
            Repair => vec![
                (RepairRate, 0.05, 100),
                (RepairCost, 0.05, 25),
                (EnergyCost, 0.04, 2_600_000),
                (DataCost, 0.02, 300_000)],
            Reclaim => vec![
                (ReclaimRate, 0.00, 100),
                (ReclaimLoss, 0.05, 3),
                (ReclaimCost, 0.05, 25),
                (EnergyCost, 0.04, 3_200_000),
                (DataCost, 0.02, 400_000)],
            Melee => vec![
                (MeleeDamage, 0.00, 200),
                (MeleeRolls, -0.15, 2),
                (MeleeAccuracy, -0.20, 0),
                (MeleeCost, 0.00, 60),
                (RotationTimeout, 0.00, 1),
                (MoveTimeout, 0.00, 4),
                (EnergyCost, 0.04, 10_000_000),
                (DataCost, 0.02, 800_000)],
            Ranged => vec![
                (RangedDamage, 0.00, 100),
                (RangedRolls, -0.08, 4),
                (RangedAccuracy, -0.10, 0),
                (RangedCost, 0.00, 60),
                (RangedData, 0.00, 20),
                (PassiveCost, 0.00, 6),
                (MoveTimeout, 0.00, 4),
                (EnergyCost, 0.04, 24_000_000),
                (DataCost, 0.02, 6_000_000)],
            Aim => vec![
                (MeleeAccuracy, 0.30, 0),
                (MeleeCost, 0.10, 0),
                (RangedAccuracy, 0.50, 0),
                (RangedCost, 0.10, 0),
                (PassiveCost, 0.00, 10),
                (PassiveData, 0.00, 1),
                (RotationTimeout, 0.00, 1),
                (MoveTimeout, 0.00, 4),
                (EnergyCost, 0.04, 8_000_000),
                (DataCost, 0.02, 880_000)],
            Armor => vec![
                (MaxHP, 0.05, 100_000),
                (RotationTimeout, 0.00, 1),
                (MoveTimeout, 0.00, 3),
                (RotationCost, 0.00, 5),
                (MoveCost, 0.00, 6),
                (EnergyCost, 0.01, 100_000),
                (DataCost, 0.005, 100_000)],
            Shield => vec![
                (PassiveCost, 0.00, 20),
                (MaxSP, 0.10, 100_000),
                (SPRegen, -0.02, 10),
                (SPRegenCost, 0.01, 1),
                (MoveTimeout, 0.00, 2),
                (EnergyCost, 0.01, 500_000),
                (DataCost, 0.005, 200_000)],
            CPU => vec![
                (PassiveCost, 0.00, 20),
                (PassiveData, 0.00, 10),
                (EnergyCost, 0.04, 1_000_000_000),
                (DataCost, 0.02, 300_000_000)],
        }
    }
}


/// A set of modules which modify the statistics of agents.
#[derive(Debug, Clone)]
pub struct ModuleList{
    /// The list of modules present and the number of each of them.
    modules: HashMap<Module, u16>,
    /// The percent change and the incrimental change. Note the percent should be correct for direct
    /// multiplication, that is, a -5% change will have been converted to 0.95.
    effects: HashMap<BaseStat, (f64, i64)>
}

impl Deref for ModuleList {
    type Target = HashMap<Module, u16>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.modules
    }
}

impl ModuleList {
    fn module_count(&self) -> u16 {
        self.modules.iter().fold(0u16, |acc, (_, &c)| acc + c)
    }

    fn add_module(&mut self, module: Module) {
        let count = self.modules.get(&module)
            .cloned()
            .unwrap_or(0);
        self.modules.insert(module, count + 1);
        for (stat, pct, inc) in module.get_effects() {
            let (l_pct, l_inc) = self.effects.get(&stat)
                .cloned()
                .unwrap_or((1.0f64, 0i64));
            self.effects.insert(
                stat,
                (l_pct * (1.0f64 + pct as f64), l_inc + inc)
            );
        }
    }

    fn effects(&self) -> &HashMap<BaseStat, (f64, i64)> {
        &self.effects
    }
}