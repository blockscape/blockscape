use super::*;


/// Pathways which aid in bot movement.
mod road;
pub use self::road::*;

/// Transfers +/- charge between buildings.
mod power_line;
pub use self::power_line::*;

/// Allows the transfer between plots.
mod power_tunnel;
pub use self::power_tunnel::*;

/// Transfers data between buildings.
mod data_bus;
pub use self::data_bus::*;

/// Allows transfer of data between plots.
mod data_tunnel;
pub use self::data_tunnel::*;


/// Stores a small amount of charge.
mod capacitor;
pub use self::capacitor::*;

/// Stores a large amount of charge.
pub struct Battery;
/// Stores a small amount of data (with electric cost).
pub struct RAMCard;
/// Stores a large amount of data.
pub struct NANDCard;

/// Data multiplier, takes data and power to make more data.
pub struct IntegratedCircuit;
/// A defense system which
pub struct FireWall;
/// Constructs and repairs robots.
pub struct Assembler;
/// Allows querying information from other plots and to send active messages.
pub struct Beacon;

/// Capable of using modules to do things (controlled by CPU).
pub struct Turret;
/// The structure which
pub struct CPU;