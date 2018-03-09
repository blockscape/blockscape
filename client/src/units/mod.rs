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
mod battery;
pub use self::battery::*;

/// Stores a small amount of data (with electric cost).
mod ram_card;
pub use self::ram_card::*;

/// Stores a large amount of data.
mod nand_card;
pub use self::nand_card::*;


/// Data multiplier, takes data and power to make more data.
mod integrated_circuit;
pub use self::integrated_circuit::*;

/// A defense system which blocks enemy units.
mod fire_wall;
pub use self::fire_wall::*;

/// Constructs and repairs robots.
mod assembler;
pub use self::assembler::*;

/// Allows querying information from other plots and to send/receive active messages.
mod beacon;
pub use self::beacon::*;

/// Capable of using modules to do things (controlled by CPU).
pub struct Turret;
/// The structure which
pub struct CPU;