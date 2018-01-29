use std::sync::Arc;
use std::time::Duration;
use futures::prelude::*;
use futures::future;
use rand::random;
use tokio_core::reactor::Timeout;
use tokio_core::reactor::Handle;
use bincode;

use forging::{BlockForger, Error};
use record_keeper::RecordKeeper;
use primitives::block::Block;

pub struct FlowerPicking {
    /// A reference to RecordKeeper so block generation/preparation can happen
    rk: Arc<RecordKeeper>,

    /// A reference to the event loop so that jobs can be dispatched
    handle: Handle,

    /// The number of milliseconds between blocks to aim for. The difficulty adjusts around this value
    rate_target: u64,

    /// How often to recalculate the difficulty, in number of blocks
    recalculate_blocks: u64
}

impl FlowerPicking {
    pub fn new(rk: Arc<RecordKeeper>, handle: Handle, rate_target: u64, recalculate_blocks: u64) -> FlowerPicking {
        FlowerPicking {
            rk: rk,
            handle: handle,
            rate_target: rate_target,
            recalculate_blocks: recalculate_blocks
        }
    }

    fn gen_rand_mod(&self, diff: u64) -> Duration {
        Duration::from_millis(random::<u64>() % (diff * self.rate_target * 2))
    }

    fn calculate_expected_difficulty(&self, block: &Block) -> Result<u64, Error> {
        let height = try!(self.rk.get_block_height(&block.header.prev)
            .map_err(|e| Error(format!("Could not get a block height: {}", e).into()))) + 1;
        
        if height % self.recalculate_blocks != 0 {

            /*let pb = self.rk.get_block(&block.header.prev);

            if pb.is_err() {
                return Box::new(future::err(Error(format!("Could not get a block from db: {}", ph.unwrap_err()).into())));
            }*/

            Ok(bincode::deserialize(
                &try!(self.rk.get_block(&block.header.prev)
                    .map_err(|e| Error(format!("Could not get a block from db: {}", e).into()))).header.blob).unwrap())
        }
        else {

            debug!("Recalculating difficulty!");

            let mut n = self.recalculate_blocks;
            if height == self.recalculate_blocks {
                // we dont want to walk all the way back to genesis
                n -= 2;
            }

            // the best way to find this block is to walk back recalculate_blocks
            let mut hash_cur = block.header.prev;

            let pb = try!(self.rk.get_block(&hash_cur)
                    .map_err(|e| Error(format!("Could not get a block from db: {}", e).into())));
            
            for _ in 1..n {
                hash_cur = try!(self.rk.get_block(&hash_cur)
                    .map_err(|e| Error(format!("Could not get a block from db: {}", e).into()))).header.prev;
            }

            // how long *should* it have taken to get to this point?
            let expected: f64 = self.rate_target as f64 * n as f64;

            let b = try!(self.rk.get_block(&hash_cur)
                    .map_err(|e| Error(format!("Could not get a block from db: {}", e).into())));

            let actual = b.header.timestamp.diff(&pb.header.timestamp).millis() as f64;

            let last_diff = bincode::deserialize::<u64>(&pb.header.blob).unwrap() as f64;

            debug!("Expected: {}, Actual: {}, Last Diff: {}", expected / 1000.0, actual / 1000.0, last_diff);

            Ok(((expected / actual) * last_diff) as u64)
        }
    }
}

impl BlockForger for FlowerPicking {

    fn create(&self, mut block: Block) -> Box<Future<Item=Block, Error=Error>> {
        let diff = tryf!(self.calculate_expected_difficulty(&block));

        block.blob = bincode::serialize(&diff, bincode::Bounded(8)).unwrap();

        // now artificially induce time for the block to become available
        let rand_mod = self.gen_rand_mod(diff);

        debug!("Scheduled block gen (diff {}): {:?}", diff, rand_mod);

        Box::new(Timeout::new(rand_mod, &self.handle).unwrap()
            .map(|_| block)
            .map_err(|e| Error(format!("Could not set timeout: {}", e))))
    }

    fn validate(&self, block: &Block) -> Option<Error> {

        // check that the difficulty matches what we expect
        let diff = self.calculate_expected_difficulty(block).expect("Database not working when validating block!");

        if let Ok(b_diff) = bincode::deserialize::<u64>(&block.header.blob) {
            if b_diff == diff {
                return None;
            }
        }

        // the flower picker always accepts any generated block
        Some(Error("Block has invalid difficulty blob".into()))
    }
}