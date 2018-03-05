pub mod flower_picking;

use futures::prelude::*;

use primitives::Block;

#[derive(Debug)]
pub struct Error(String);

pub trait BlockForger {
    fn create(&self, block: Block) -> Box<Future<Item=Block, Error=Error>>;
    fn validate(&self, block: &Block) -> Option<Error>;
}