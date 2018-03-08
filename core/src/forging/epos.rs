use std::sync::Arc;
use std::time::Duration;
use std::collections::VecDeque;
use futures::prelude::*;
use futures::future;
use futures::sync::*;
use rand::random;
use tokio_core::reactor::Timeout;
use tokio_core::reactor::Remote;
use bincode;

use forging::{BlockForger, Error};
use record_keeper::RecordKeeper;
use primitives::block::Block;

const EPOS_BROADCAST_ID: u8 = 0;

/// Configuration for the proof of stake algorithm
pub struct EPoSConfig {
    /// The number of milliseconds between blocks to aim for. The difficulty adjusts around this value
    rate_target: u64,

    /// How often to recalculate the difficulty, in number of blocks
    recalculate_blocks: u64,

    /// The number of blocks to scan to count "active" validators, used as result for validators_count_base below
    validators_scan: u64,

    /// The number of validator keys include in the hash "random" source for validator selection
    hash_compounds: u64,

    /// The base of an exponential function to determine the number of validators to require. For example, if this number is 10, then 100 validators are needed for 2 signatures required.
    /// A good reasonable value for this is 4.
    validators_count_base: u64,

    /// The number of blocks a validator must wait before participating again. TODO: To what extent should this rule apply?
    validator_cooldown: u64
}

type EPoSSignature = (Vec<u8>, U256);

/// Data which is associated with signing and blobbing a block
#[derive(Serialize, Deserialize, Debug)]
struct EPoSBlockData {
    /// Hashes of validator's signatures in previous blocks
    hashes: Vec<U256>,

    /// Signatures we have so far for this block
    sigs: Vec<EPoSSignature>
}

impl EPoSBlockData {

    fn new(hashes: Vec<U160>, sigs: Vec<(Vec<u8>, U256)>) -> EPoSBlockData {
        EPoSBlockData {
            hashes: hashes,
            sigs: sigs
        }
    }

    /// Apply this data to the block, by setting signatures and applying my signature. Will return the fitness for the random seed, used to calculate submit time.
    /// If any signature or check comes out invalid, an error is returned
    pub fn apply_block(block: &mut Block, hashes: &Vec<U256>, my_signer: &PKey) -> Result<u64, Error> {
        // sign the serialized data of our self
        let block_data = if block.blob.len() {
            bincode::deserialize(&block.blob[..])
                .map_err(|e| Error(format!("Could not deserialize block blob: {}", e).into()))?
        }
        else {
            EPoSBlockData {
                hashes: hashes.clone(),
                sigs: Vec::with_capacity(1)
            }
        }

        block_data.sigs.push((
            my_signer.public_key_to_der(),
            signer::sign_bytes(&block.blob[..], my_signer)
        ));

        block.blob = bincode::serialize(&block_data);

        Ok(calculate_fitness(block))
    }

    /// Simultaneously verifies that block signatures are valid, and returns the fitness value
    pub fn calculate_fitness(block: &Block, hashes: &Vec<U256>) -> Result<u64, Error> {
        let block_data = bincode::deserialize(&block.blob[..])
            .map_err(|e| Error(format!("Could not deserialize block blob: {}", e).into()))?;

        let hashes_hash = hash::hash_obj(hashes)

        let sig_so_far = EPoSBlockData {
            hashes: hashes,
            sigs: vec![block_data[0]]
        }

        let fitness: u64 = 0;

        while sig_so_far.sigs.len() < block_data.sigs.len() {

            let csig = block_data.sigs[sigs_so_far.sigs.len()];

            if !signer::verify_obj(sig_so_far, csig.1, csig.0) {
                return Error(format!("Signature for signed block not valid!").into());
            }

            sig_so_far.sigs.push(csig);

            let h = hash::hash_obj(&sig_so_far).0[0];

            fitness += if h > hashes_hash {
                h - hashes_hash
            }
            else {
                hashes_hash - h
            } / block_data.sigs.len();
        }

        Ok(fitness)
    }

    pub fn get_relevant_validator(block: &BlockHeader) -> Result<u64, Error> {
        let block_data = bincode::deserialize(&block.blob[..])
            .map_err(|e| Error(format!("Could not deserialize block blob: {}", e).into()))?;

        hash::hash_pub_key(block_data.sigs[block_data.sigs.len() / 2])
    }
}

/// "Enhanced" Proof of Stake implementation which is a hardened PoS resistant to differential cryptoanalysis and the halting problem
pub struct EPoS {
    /// A reference to RecordKeeper so block generation/preparation can happen
    rk: Arc<RecordKeeper>,

    /// A reference to the event loop so that jobs can be dispatched
    remote: Remote,

    /// The configuration for EPoS
    config: EPoSConfig
}

impl EPoS {
    pub fn new(rk: Arc<RecordKeeper>, handle: Handle, config: EPoSConfig) -> EPoS {

        let hash_compounds = config.hash_compounds;

        EPoS {
            rk: rk,
            handle: handle,
            config: config,
            hashes: VecDeque::with_capacity(hash_compounds)
        }
    }

    fn gen_rand_mod(&self, diff: u64) -> Duration {
        if diff == 0 {
            Duration::from_secs(1)
        }
        else {
            Duration::from_millis(random::<u64>() % (diff * self.config.rate_target * 2))
        }
    }

    /// Calculates the number of validator signatures required to complete a block
    fn calculate_required_validators(&self, prev: &U256) -> Result<u64, Error> {
        // First, update the validator hashes (we only look at the MIDDLE one in each block since it is the hardest to grind)
        let blocks = Vec::with_capacity(self.config.validators_scan);
        for i in 0..self.config.validators_scan() {
            blocks.push(self.rk.get_block_header(blocks[blocks.len() - 1]
                .map_err(|e| Error(format!("Could not get a block from db: {}", e).into()))?.prev));
        }

        let validators: HashSet<U160> = HashSet::new();

        for block in blocks {
            validators.insert(EPoSBlockData::get_relevant_validator(&block));
        }

        (validators.len() as f64).log(self.config.validators_count_base).trunc() as u64
    }

    /// Calculates the actual block difficulty, taking into account the current level of validators required and etc.
    fn calculate_expected_difficulty(&self, block: &Block) -> Result<u64, Error> {
        let height = try!(self.rk.get_block_height(&block.header.prev)
            .map_err(|e| Error(format!("Could not get a block height: {}", e).into()))) + 1;
        
        let base_diff = if height % self.recalculate_blocks != 0 {

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

        // divide the base difficulty by the number of validators required to mine a block
        base_diff / calculate_required_validators()
    }

    fn evaluate_block(block: Block) -> bool {
        // try to add our signature onto this block
        let res = EPoSBlockData::apply_block(&mut block, hashes, self.forge_key);
        if let Err(e) = res {
            warn!("Block check was not valid: {}, {}", e, block);
            return false;
        }

        let cur_diff = res.unwrap();

        // calculate difficulty, consider dispatchment
        let res = self.calculate_expected_difficulty(&block)
        if let Err(e) = res {
            warn!("Difficulty calculation failed: {}", e);
            return false;
        }

        let exp_diff = res.unwrap();

        // the ratio of these determines when we are able to publish the block
        
    }
}

impl BlockForger for EPoS {

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

impl BroadcastReceiver for EPoS {
    /// Returns a unique identifier to separate events for this broadcast ID. Must be unique per application.
    pub get_broadcast_id() {
        EPOS_BROADCAST_ID
    }

    /// Called when a broadcast is received. If the broadcast is to be propogated, the broadcast event must be re-called.
    /// Internally, network automatically handles duplicate events as a result of the reliable flood, so that can be safely ignored
    pub receive_broadcast(network_id: &U256, payload: &Vec<u8>) -> bool {
        if let Ok(block) = bincode::deserialize(payload[..]) {
            self.evaluate_block(block);
        }

        false
    }
}