use std::sync::Arc;
use std::time::Duration;
use futures::prelude::*;
use futures::future;
use rand::random;
use tokio_core::reactor::Timeout;
use bincode;

use forging::{BlockForger, Error};
use record_keeper::RecordKeeper;
use primitives::block::Block;
use time::Time;

pub struct FlowerPicking {
    rk: Arc<RecordKeeper>,
    rate_target: u64,
    recalculate_blocks: u64
}

impl FlowerPicking {
    pub fn new(rk: Arc<RecordKeeper>, rate_target: u64, recalculate_blocks: u64) -> FlowerPicking {
        FlowerPicking {
            rk: rk,
            rate_target: rate_target,
            recalculate_blocks: recalculate_blocks
        }
    }
}

impl BlockForger for FlowerPicking {

    fn create(&self, block: Block) -> Box<Future<Item=Block, Error=Error>> {
        
        // what height will this block be?

        let ph = self.rk.get_block_height(&block.header.prev);

        if ph.is_err() {
            return Box::new(future::err(Error(format!("Could not get a block height: {}", ph.unwrap_err()).into())));
        }

        let height = ph.unwrap() + 1;

        let diff: u64 = if height % self.recalculate_blocks == 0 {

            let pb = self.rk.get_block(&block.header.prev);

            if pb.is_err() {
                return Box::new(future::err(Error(format!("Could not get a block from db: {}", ph.unwrap_err()).into())));
            }

            bincode::deserialize(&pb.unwrap().header.blob).unwrap()
        }
        else {
            // the best way to find this block is to walk back recalculate_blocks
            let mut hash_cur = block.header.prev;
            
            for n in 1..self.recalculate_blocks {
                let pb = self.rk.get_block(&block.header.prev);

                if pb.is_err() {
                    return Box::new(future::err(Error(format!("Could not get a block from db: {}", ph.unwrap_err()).into())));
                }

                hash_cur = pb.unwrap().header.prev;
            }

            // how long *should* it have taken to get to this point?
            let expected = self.rate_target * self.recalculate_blocks;
            let pb = self.rk.get_block(&block.header.prev);

            if pb.is_err() {
                return Box::new(future::err(Error(format!("Could not get a block from db: {}", ph.unwrap_err()).into())));
            }

            let b = pb.unwrap();

            let actual = b.header.timestamp.diff(&Time::current()).millis() as u64;

            (expected / actual) * bincode::deserialize(&b.header.blob).unwrap()
        };

        block.blob = bincode::serialize(&diff, bincode::Bounded(8)).unwrap();

        // now artificially induce time for the block to become available
        let rand_mod = gen_rand_mod(diff);

        Box::new(Timeout::new(rand_mod, &core.handle()).unwrap())
    }

    fn validate(&self, block: &Block) -> Option<Error> {
        // the flower picker always accepts any generated block
        return None;
    }
}

fn gen_rand_mod(diff: u64) -> Duration {
    Duration::from_secs(random::<u64>() % diff)
}