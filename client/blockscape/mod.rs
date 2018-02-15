mod agent;
mod scripts;
mod structure;
mod world;

use blockscape_core::primitives::Coord;
use std::collections::HashMap;

/// Something all things should implement if information about them can be displayed in a UI.
pub trait Describable {
    /// Internal, unique ID per definition (not instance).
    fn id() -> u8;
    /// Internal, unique ID per definition (not instance). E.g. "data_processor_t1".
    fn str_id() -> &'static str;
    /// Human-readable name of the defined type, such as "Data Processor".
    fn object_name() -> &'static str;
    /// Name of this worldly instance if relevant.
    fn instance_name() -> Option<String>;
    /// A message about what it is currently doing or what state it is in.
    fn status() -> Option<String>;
    /// A description of what this object is.
    fn description() -> &'static str;
}

// Something all things in Blockscape should implement if they can exist in the world.
pub trait Worldly: Describable {
    /// Location of this worldly object on it's plot. If it occupies multiple tiles on a plot, this
    /// will be the top-left corner.
    fn location() -> Coord;

    /// Current number of hit-points the object has.
    fn hp() -> u32;
    /// Maximum number of hit-points the object may have.
    fn max_hp() -> u32;
    /// Rate at which the hit-points naturally decay in hp/tick.
    fn decay_rate() -> f32;

    /// Current shield strength (shield-points).
    fn sp() -> u32;
    /// Maximum number of shield-points.
    fn max_sp() -> u32;
    /// Rate at which shield-points may be regenerated in sp per tick.
    fn sp_regen_rate() -> u32;
    /// Cost to regenerate shield in charge per shield-point.
    fn sp_regen_cost() -> f32;


    /// Current positive charge carried.
    fn p_charge() -> u64;
    /// Current negative charge carried.
    fn n_change() -> u64;
    /// Maximum charge which can be carried. The following must be true:
    /// `(p_charge + n_charge) < max_charge.`
    fn max_charge() -> u64;
    /// The maximum amount of charge which may be brought in and or sent out in a tick.
    fn charge_rate() -> u64;
    /// Energy required per tick when no actions are being performed.
    fn passive_energy_cost() -> f32;

    /// The amount of data stored by this object.
    fn data() -> u64;
    /// The maximum amount of data which can be stored by this object.
    fn max_data() -> u64;
    /// The maximum amount of data which may be brought in and or sent out in a tick.
    fn transfer_rate() -> u64;

    /// Energy cost to build this object.
    fn energy_cost() -> u64;
    /// Data cost to build this object.
    fn data_cost() -> u64;
}

/// An object which is stationary.
pub trait Structure: Worldly {
    /// Determines the build menu
    fn category() -> &'static str;

    /// The length of this object in the x-direction.
    fn x_len() -> u32;
    /// The length of this object in the y-direction.
    fn y_len() -> u32;
}

/// And object which is controlled by a CPU.
pub trait Agent<'a>: Worldly {
    /// Retrieve the modules installed in this agent.
    fn modules() -> &'a ModuleList;
    /// Maximum number of modules which can be installed in this agent.
    fn max_modules() -> u16;

    /// Amount of charge it siphon per tick; should be <= `charge_rate()`.
    fn siphon_rate() -> u64;
    /// Percent of data which is lost per tick when mining (subtracted from siphon rate).
    fn siphon_loss() -> f32;
    /// Amount of energy required per tick to siphon energy.
    fn siphon_cost() -> u64 { 0u64 }

    /// Amount of data which can be mined per tick; should be <= `transfer_rate()`.
    fn mining_rate() -> u64;
    /// Percent of data which is lost per tick when mining (subtracted from mining rate).
    fn mining_loss() -> f32;
    /// Amount of energy required per tick to mine data.
    fn mining_cost() -> u64;

    /// Amount of HP per tick which can be constructed.
    fn build_rate() -> u32;
    /// Amount of energy required per tick to build (in addition to the building's cost).
    fn build_cost() -> u64;

    /// Amount of HP per tick which can be repaired.
    fn repair_rate() -> u32;
    /// Amount of energy required per tick to repair.
    fn repair_cost() -> u64;

    /// Amount of HP per tick which can be destroyed (of non-enemy structure).
    fn reclaim_rate() -> u32;
    /// Percent of resources which are lost per HP reclaimed.
    fn reclaim_loss() -> f32;
    /// Amount of energy required per tick to repair.
    fn reclaim_cost() -> u64;

    /// Damage per tick which can be dealt by a successful melee attack.
    fn melee_damage() -> u32;
    /// Number of individual attacks which can be rolled for in a tick. Each can perform
    /// `melee_damage() / melee_rolls()` of damage per tick if they are successful.
    fn melee_rolls() -> u8;
    /// Chance to hit with a given attack on a tick.
    fn melee_accuracy() -> f32;
    /// Energy cost to attempt an attack each turn.
    fn melee_cost() -> u64;

    /// Damage per tick which can be dealt by a successful ranged attack.
    fn ranged_damage(distance: u32) -> u32;
    /// Number of individual attacks which can be rolled for in a tick. Each can perform
    /// `ranged_damage() / ranged_rolls()` of damage per tick if they are successful.
    fn ranged_rolls() -> u8;
    /// Chance to hit with a given attack on a tick.
    fn ranged_accuracy(distance: u32) -> f32;
    /// Energy cost to attempt an attack each turn.
    fn ranged_cost(distance: u32) -> u64;
    /// Data cost to attempt an attack each turn.
    fn ranged_data(distance: u32) -> u64;
}

pub trait Mobile<'a>: Agent<'a> {
    /// Direction the agent is currently facing.
    fn orientation() -> Direction;
    /// The number of ticks that must be waited between rotations. (Zero is valid)
    fn max_rotation_speed() -> u16;
    /// Charge cost to rotate 45 degrees.
    fn rotation_cost() -> u64;

    fn stationary() -> bool;
    /// The number of ticks that must be waited between moves. This number is increased by a factor
    /// of sqrt(2) + 1 when traveling diagonally. (Zero is valid).
    fn max_speed() -> u16;
    /// Charge cost to move one tile. Increased by a factor of sqrt(2) + 1 when traveling
    /// diagonally.
    fn move_cost() -> u64;

    // fn destination() -> (PlotID, Coord);
    // fn path();
}

pub enum Direction {
    N, NE, E, SE, S, SW, W, NW
}

struct ModuleList(Vec<(Module, u8)>);

pub enum Module {
    Move, Capacitor, Memory, Siphon, IOPort, Overclock, DataBus, Construction, Repair, Reclaim,
    Melee, Ranged, Aim, Armor, Shield, CPU
}