use std::cmp::min;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use futures::prelude::*;
use futures::sync::*;
use tokio_core::reactor::Handle;
use bincode;
use crypto::sha3::Sha3;
use crypto::digest::Digest;
use openssl::pkey::PKey;

use forging::{BlockForger, ForgeError};
use record_keeper::RecordKeeper;
use network::client::BroadcastReceiver;
use primitives::block::{Block, BlockHeader};
use primitives::{U256, U160};
use time::Time;
use signer;
use hash;

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
    //hash_compounds: u64,

    /// The base of an exponential function to determine the number of validators to require. For example, if this number is 10, then 100 validators are needed for 2 signatures required.
    /// A good reasonable value for this is 4.
    validators_count_base: u64,

    /// The number of blocks a validator must wait before participating again. TODO: To what extent should this rule apply?
    //validator_cooldown: u64,

    /// Signing private key(s) for us to participate in the forge
    signing_keys: Vec<Vec<u8>>
}

type EPoSSignature = (Vec<u8>, U256);

/// Data which is associated with signing and blobbing a block
#[derive(Serialize, Deserialize, Debug)]
struct EPoSBlockData {
    sigs: Vec<EPoSSignature>
}

impl EPoSBlockData {

    /// Apply this data to the block, by setting signatures and applying my signature. Will return the EPoS data for evaluation.
    /// If any signature or check comes out invalid, an error is returned
    pub fn apply_block(block: &mut Block, my_signer: &PKey) -> Result<EPoSBlockData, ForgeError> {
        // sign the serialized data of our self
        let mut block_data = if !block.blob.is_empty() {
            bincode::deserialize(&block.blob[..])
                .map_err(|e| ForgeError(format!("Could not deserialize block blob: {}", e).into()))?
        }
        else {
            EPoSBlockData {
                sigs: Vec::with_capacity(1)
            }
        };

        block_data.sigs.push((
            my_signer.public_key_to_der().map_err(|_| ForgeError(format!("Could not convert public key to DER")))?,
            bincode::deserialize(&signer::sign_bytes(&block.blob[..], my_signer)).expect("Could not convert key signature to u256")
        ));

        block.blob = bincode::serialize(&block_data, bincode::Infinite).map_err(|_| ForgeError(format!("Could not serialize generated block data")))?;

        Ok(block_data)
    }

    pub fn decode_relevant_validation_data(block: &BlockHeader) -> Result<(U160, U256), ForgeError> {
        let block_data = bincode::deserialize::<EPoSBlockData>(&block.blob[..])
            .map_err(|e| ForgeError(format!("Could not deserialize block blob: {}", e).into()))?;

        Ok((hash::hash_pub_key(&block_data.sigs[block_data.sigs.len() / 2].0), block_data.sigs[block_data.sigs.len() / 2].1))
    }

    pub fn get_relevant_validation_data(&self) -> (U160, U256) {
        (hash::hash_pub_key(&self.sigs[self.sigs.len() / 2].0), self.sigs[self.sigs.len() / 2].1)
    }
}

/// "Enhanced" Proof of Stake implementation which is a hardened PoS resistant to differential cryptoanalysis and the halting problem
pub struct EPoS {
    /// A reference to RecordKeeper so block generation/preparation can happen
    rk: Arc<RecordKeeper>,

    /// the sending end of a future dispatch for when a block is found
    on_block: Mutex<Option<oneshot::Sender<Block>>>,

    /// An internal tracker for the currently known best block we can submit
    /// The first value is the time at which it should be submitted, the second is the block to submit
    best_block: Arc<Mutex<Option<(u64, Block, u64)>>>,

    /// OpenSSL private key(s) used for signing blocks
    keys: Vec<(U160, PKey)>,

    /// The configuration for EPoS
    config: EPoSConfig
}

impl EPoS {
    pub fn new(rk: Arc<RecordKeeper>, _handle: Handle, config: EPoSConfig) -> Result<EPoS, ForgeError> {
        let keys: Vec<(U160, PKey)> = config.signing_keys.iter().map(|raw_data| {
            let k = PKey::private_key_from_der(raw_data).map_err(|e| ForgeError(format!("Could not decode private key: {:?}", e)))?;
            let d = k.public_key_to_der().unwrap();

            Ok((hash::hash_pub_key(&d), k))
        }).collect::<Result<Vec<_>, ForgeError>>()?;

        Ok(EPoS {
            rk: rk,
            on_block: Mutex::new(None),
            best_block: Arc::new(Mutex::new(None)),
            keys: keys,
            config: config,
        })
    }

    /// Calculate how long a node should wait until it propogates a block. This is based on a relationship between the current difficulty and 
    fn gen_wait(&self, diff: u64, stake: u64, target: u64, actual: u64) -> u64 {

        if stake == 0 {
            return u64::max_value();
        }

        let delta = min(target.wrapping_sub(actual), actual.wrapping_sub(target));

        // range basically represents the number of units of delta per millisecond of wait
        let range = u64::max_value() / (diff + 1) / self.config.rate_target;

        delta / range / stake
    }

    /// Calculates the validator target hash, and the number of validators required to validate
    fn calculate_validator_info(&self, prev: &U256) -> Result<(U256, u64), ForgeError> {
        // First, update the validator hashes (we only look at the MIDDLE one in each block since it is the hardest to grind)
        let mut blocks = Vec::with_capacity(self.config.validators_scan as usize);
        {
            let mut p = *prev;
            for _ in 0..self.config.validators_scan {
                blocks.push(self.rk.get_block_header(&p).map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into()))?);

                p = blocks[blocks.len() - 1].prev;
            }
        }

        let mut validators: HashSet<U160> = HashSet::new();

        let mut buf = [0u8; 32];
        let mut hasher = Sha3::sha3_256();

        for block in blocks {
            let (validator_id, sig) = EPoSBlockData::decode_relevant_validation_data(&block)?;

            sig.to_big_endian(&mut buf);
            hasher.input(&buf);

            validators.insert(validator_id);
        }

        hasher.result(&mut buf); //don't care about first hash, only the second
        hasher.reset();
        hasher.input(&buf);
        hasher.result(&mut buf);

        Ok((U256::from_big_endian(&mut buf), (validators.len() as f64).log(self.config.validators_count_base as f64).trunc() as u64))
    }

    /// Calculates the actual block difficulty, taking into account the current level of validators required and etc.
    fn calculate_expected_difficulty(&self, block: &Block) -> Result<u64, ForgeError> {
        let height = try!(self.rk.get_block_height(&block.header.prev)
            .map_err(|e| ForgeError(format!("Could not get a block height: {}", e).into()))) + 1;
        
        let base_diff = if height % self.config.recalculate_blocks != 0 {

            /*let pb = self.rk.get_block(&block.header.prev);

            if pb.is_err() {
                return Box::new(future::err(ForgeError(format!("Could not get a block from db: {}", ph.unwrap_err()).into())));
            }*/

            Ok(bincode::deserialize(
                &try!(self.rk.get_block(&block.header.prev)
                    .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into()))).header.blob).unwrap())
        }
        else {

            debug!("Recalculating difficulty!");

            let mut n = self.config.recalculate_blocks;
            if height == self.config.recalculate_blocks {
                // we dont want to walk all the way back to genesis
                n -= 2;
            }

            // the best way to find this block is to walk back recalculate_blocks
            let mut hash_cur = block.header.prev;

            let pb = try!(self.rk.get_block(&hash_cur)
                    .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into())));
            
            for _ in 1..n {
                hash_cur = try!(self.rk.get_block(&hash_cur)
                    .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into()))).header.prev;
            }

            // how long *should* it have taken to get to this point?
            let expected: f64 = self.config.rate_target as f64 * n as f64;

            let b = try!(self.rk.get_block(&hash_cur)
                    .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into())));

            let actual = b.header.timestamp.diff(&pb.header.timestamp).millis() as f64;

            let last_diff = bincode::deserialize::<u64>(&pb.header.blob).unwrap() as f64;

            debug!("Expected: {}, Actual: {}, Last Diff: {}", expected / 1000.0, actual / 1000.0, last_diff);

            Ok((expected * last_diff / actual) as u64)
        };

        base_diff
    }

    /// Tries to add our own signature to a received block, and prepare it for transmission if it is a keeper.
    fn evaluate_block(&self, block: Block) -> bool {
        // try to add one of our signatures onto this block
        // TODO: This could be much more efficient

        let vi = self.calculate_validator_info(&block.prev);

        if let Err(e) = vi {
            // TODO: Change logging strategy?
            warn!("Forge validator calculation failed: {:?}", e);
            return false;
        }

        let (target, req_validators) = vi.unwrap();

        // calculate difficulty, consider dispatchment
        let res = self.calculate_expected_difficulty(&block);
        if let Err(e) = res {
            warn!("Difficulty calculation failed: {:?}", e);
            return false;
        }

        let exp_diff = res.unwrap();

        let mut cur_best: Option<(Block, u64)> = None;

        for &(ref key_id, ref key) in &self.keys {
            let mut key_block = block.clone();

            let res = EPoSBlockData::apply_block(&mut key_block, &key);
            if let Err(e) = res {
                warn!("Block check was not valid: {:?}, {:?}", e, key_block);
                return false;
            }
            let actual = res.unwrap().get_relevant_validation_data().1;

            // get the stake of the account we are forging
            let res = self.rk.get_account_value(&key_id);
            if let Err(e) = res {
                warn!("Could not get value held in account: {:?}", e);
                return false;
            }

            let stake = res.unwrap();

            let w = self.gen_wait(exp_diff, stake, target.into(), actual.into());

            if cur_best.is_some() {
                let cb = cur_best.as_mut().unwrap();
                if w < cb.1 {
                    *cb = (key_block, w);
                }
            }
            else {
                cur_best = Some((key_block, w));
            }
        }

        if let Some(best) = cur_best {
            let disp = block.timestamp.millis() as u64 + best.1;

            // update timeouts
            let mut prev_best = self.best_block.lock().unwrap();

            if prev_best.is_some() {
                let pb = prev_best.as_mut().unwrap();
                if disp < pb.0 {
                    // update block and timeout
                    *pb = (disp, best.0, req_validators);
                }
            }
            else {
                *prev_best = Some((disp, best.0, req_validators));
            }
        }

        true
    }
}

impl BlockForger for EPoS {

    fn create(&self, block: Block) -> Box<Future<Item=Block, Error=ForgeError>> {
        let (tx, rx) = oneshot::channel();
        *self.on_block.lock().unwrap() = Some(tx);

        self.evaluate_block(block);
        Box::new(rx.map_err(|_| ForgeError(format!("Cancelled forge!"))))
    }

    fn validate(&self, block: &Block) -> Option<ForgeError> {

        // check that the difficulty matches what we expect
        let diff = self.calculate_expected_difficulty(block).expect("Database not working when validating block!");

        if let Ok(b_diff) = bincode::deserialize::<u64>(&block.header.blob) {
            if b_diff != diff {
                return Some(ForgeError(format!("Block difficulty is invalid")));
            }
        }
        else {
            return Some(ForgeError(format!("Block blob decode error!")));
        }

        // ensure that the registered time of the block is far enough ahead of the previous block
        if block.timestamp > Time::current() {
            return Some(ForgeError(format!("Block has been submitted too early")));
        }

        None
    }
}

impl BroadcastReceiver for EPoS {
    /// Returns a unique identifier to separate events for this broadcast ID. Must be unique per application.
    fn get_broadcast_id(&self) -> u8 {
        EPOS_BROADCAST_ID
    }

    /// Called when a broadcast is received. If the broadcast is to be propogated, the broadcast event must be re-called.
    /// Internally, network automatically handles duplicate events as a result of the reliable flood, so that can be safely ignored
    fn receive_broadcast(&self, _network_id: &U256, payload: &Vec<u8>) -> bool {
        if let Ok(block) = bincode::deserialize(&payload[..]) {
            self.evaluate_block(block);
        }

        false
    }
}

#[test]
fn block_data() {

}

#[test]
fn calculate_difficulty() {

}