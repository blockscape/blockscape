pub mod flower_picking;
pub mod epos;

use std::error::Error;
use std::fmt::Display;
use std::fmt;

use futures::prelude::*;

use primitives::Block;

#[derive(Debug)]
pub struct ForgeError(String);

impl Display for ForgeError {
    /// Print the error
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
        )
    }
}

impl Error for ForgeError {
    fn description(&self) -> &str {
        self.0.as_str()
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}

pub trait BlockForger {
    fn create(&self, block: Block) -> Box<Future<Item=Block, Error=ForgeError>>;
    fn validate(&self, block: &Block) -> Option<ForgeError>;
}