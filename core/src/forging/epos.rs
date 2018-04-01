use std::cmp::{min,max};
use std::sync::{Arc, Mutex};
use std::mem;
use std::time::Duration;
use std::collections::HashSet;
use futures::prelude::*;
use futures::sync::*;
use futures::sync::mpsc::{unbounded, UnboundedSender};
use tokio_core::reactor::{Remote,Timeout};
use bincode;
use crypto::sha3::Sha3;
use crypto::digest::Digest;
use openssl::pkey::PKey;

use forging::{BlockForger, ForgeError};
use record_keeper::RecordKeeper;
use network::client::BroadcastReceiver;
use network::client::ClientMsg;
use primitives::block::{Block, BlockHeader};
use primitives::{U256, U256_ZERO, U160};
use time::Time;
use signer;
use hash;

const EPOS_BROADCAST_ID: u8 = 0;

/// Configuration for the proof of stake algorithm
pub struct EPoSConfig {
    /// The number of milliseconds between blocks to aim for. The difficulty adjusts around this value
    pub rate_target: u64,

    /// How often to recalculate the difficulty, in number of blocks
    pub recalculate_blocks: u64,

    /// The number of blocks to scan to count "active" validators, used as result for validators_count_base below
    pub validators_scan: u64,

    /// The number of validator keys include in the hash "random" source for validator selection
    //hash_compounds: u64,

    /// The base of an exponential function to determine the number of validators to require. For example, if this number is 10, then 100 validators are needed for 2 signatures required.
    /// A good reasonable value for this is 4.
    pub validators_count_base: u64,

    /// The number of blocks a validator must wait before participating again. TODO: To what extent should this rule apply?
    //validator_cooldown: u64,

    /// Signing private key(s) for us to participate in the forge
    pub signing_keys: Vec<Vec<u8>>
}

impl EPoSConfig {
    // generate a EPoS config with reasonable defaults from the given keys.
    pub fn new(signing_keys: Vec<Vec<u8>>) -> EPoSConfig {
        EPoSConfig {
            rate_target: 12 * 1000, // 12 seconds
            recalculate_blocks: 1800, // 6 hours 
            validators_scan: 100,
            validators_count_base: 3,
            signing_keys
        }
    }
}

type EPoSSignature = (Vec<u8>, U256);

/// Data which is associated with signing and blobbing a block
#[derive(Serialize, Deserialize, Debug)]
struct EPoSBlockData {
    pub difficulty: u64, // TODO: Consider removing
    pub sigs: Vec<EPoSSignature>
}

impl EPoSBlockData {

    /// Apply this data to the block, by setting signatures and applying my signature. Will return the EPoS data for evaluation.
    /// If any signature or check comes out invalid, an error is returned
    pub fn apply_block(block: &mut Block, difficulty: u64, validator_info: &U256, my_signer: &PKey) -> Result<EPoSBlockData, ForgeError> {
        // sign the serialized data of our self
        let mut block_data = if !block.blob.is_empty() {
            bincode::deserialize(&block.blob[..])
                .map_err(|e| ForgeError(format!("Could not deserialize block blob (buffer size was {}): {}", block.blob.len(), e).into()))?
        }
        else {
            EPoSBlockData {
                difficulty: difficulty,
                sigs: Vec::with_capacity(1)
            }
        };

        let mut to_sign = bincode::serialize(validator_info, bincode::Infinite).unwrap();
        
        to_sign.extend(block.blob.clone());

        block_data.sigs.push((
            my_signer.public_key_to_der().map_err(|_| ForgeError(format!("Could not convert public key to DER")))?,
            bincode::deserialize(&signer::sign_bytes(&to_sign, my_signer)).expect("Could not convert key signature to u256")
        ));

        block.blob = bincode::serialize(&block_data, bincode::Infinite).map_err(|_| ForgeError(format!("Could not serialize generated block data")))?;

        Ok(block_data)
    }

    /// Verifies that the signatures all match for the given block data for the given validator info hash
    pub fn check_sigs(&self, validator_info: &U256) -> bool {
        let mut sig_so_far = bincode::serialize(validator_info, bincode::Infinite).unwrap();

        for sig in self.sigs.iter() {

            let res = PKey::public_key_from_der(&sig.0);

            if res.is_err() {
                return false; // TODO: Would be nice if there was a way to report this
            }

            let ser = bincode::serialize(&sig.1, bincode::Infinite).unwrap();

            if !signer::verify_bytes(&sig_so_far, &ser, &res.unwrap()) {
                return false;
            }

            sig_so_far.extend(ser);
        }

        true
    }

    pub fn decode_relevant_validation_data(block: &BlockHeader) -> Result<(U160, U256), ForgeError> {
        let block_data = bincode::deserialize::<EPoSBlockData>(&block.blob[..])
            .map_err(|e| ForgeError(format!("Could not deserialize block blob (buffer size was {}): {}", block.blob.len(), e).into()))?;

        Ok((hash::hash_pub_key(&block_data.sigs[block_data.sigs.len() / 2].0), block_data.sigs[block_data.sigs.len() / 2].1))
    }

    pub fn get_relevant_validation_data(&self) -> (U160, U256) {
        (hash::hash_pub_key(&self.sigs[self.sigs.len() / 2].0), self.sigs[self.sigs.len() / 2].1)
    }
}

struct EPoSContext {
    /// A reference to RecordKeeper so block generation/preparation can happen
    pub rk: Arc<RecordKeeper>,

    pub net: UnboundedSender<ClientMsg>,

    pub remote: Remote,

    /// An internal tracker for the currently known best block we can submit
    /// The first value is the time at which it should be submitted, the second is the block to submit
    pub best_block: Arc<Mutex<Option<(u64, Block, u64)>>>,

    /// the sending end of a future dispatch for when a block is found
    on_block: Mutex<Option<oneshot::Sender<Block>>>,
}

/// "Enhanced" Proof of Stake implementation which is a hardened PoS resistant to differential cryptoanalysis and the halting problem
pub struct EPoS {

    ctx: Arc<EPoSContext>,

    /// OpenSSL private key(s) used for signing blocks
    keys: Vec<(U160, PKey)>,

    /// The configuration for EPoS
    config: EPoSConfig
}

impl EPoS {
    pub fn new(rk: Arc<RecordKeeper>, net: UnboundedSender<ClientMsg>, remote: Remote, config: EPoSConfig) -> Result<Arc<EPoS>, ForgeError> {
        let keys: Vec<(U160, PKey)> = config.signing_keys.iter().map(|raw_data| {
            let k = PKey::private_key_from_der(raw_data).map_err(|e| ForgeError(format!("Could not decode private key: {:?}", e)))?;
            let d = k.public_key_to_der().unwrap();

            Ok((hash::hash_pub_key(&d), k))
        }).collect::<Result<Vec<_>, ForgeError>>()?;

        let pos = Arc::new(EPoS {
            ctx: Arc::new(EPoSContext {
                rk: rk,
                net: net,
                remote: remote,
                best_block: Arc::new(Mutex::new(None)),
                on_block: Mutex::new(None)
            }),
            keys: keys,
            config: config,
        });

        let pos2 = Arc::clone(&pos);

        // register ourself
        pos.ctx.net.unbounded_send(ClientMsg::RegisterBroadcastReceiver(EPOS_BROADCAST_ID, pos2)).expect("Could not register EPoS with network");

        Ok(pos)
    }

    /// Called when a waiting period has completed and a block should be sent, either as a completed block or as a incomplete block
    fn propagate_block(ctx: Arc<EPoSContext>) {

        let mut bb = ctx.best_block.lock().unwrap();

        if let Some((ref req_validators, ref block, ref propagate_at)) = *bb {

            if *propagate_at > Time::current().millis() as u64 + 10 {
                debug!("Skipping block propogate: timeout ({}) is later than current time ({})", propagate_at, Time::current().millis());
                return; // should not do anything with this yet.
            }

            // this should technically never fail because propogated blocks should only come from what we have generated internally
            let block_data = bincode::deserialize::<EPoSBlockData>(&block.blob[..])
                .expect("Unable to decode generated PoS block info");

            if block_data.sigs.len() < *req_validators as usize {
                // more signatures neeeded, use the broadcast interface in the network!
                println!("FORGE: Partial block submitted (have {}, reqd {})", block_data.sigs.len(), req_validators);

                ctx.net.unbounded_send(ClientMsg::SendBroadcast(
                    block.shard,
                    EPOS_BROADCAST_ID,
                    bincode::serialize(block, bincode::Infinite).expect("Could not serialize block!")
                )).expect("Failed to propagate partial signed block");
            }
            else {
                // block should be submitted!
                let mut sender = ctx.on_block.lock().unwrap();
                let mut other = None;

                mem::swap(&mut *sender, &mut other);

                if other.is_some() {
                    debug!("Submitting block result (reqd {} validators)!", req_validators);
                    other.unwrap().send(block.clone()).unwrap();
                }
                else {
                    warn!("Could not submit block: submission not available!");
                }

                *sender = None;
            }
        }
        // else, there is nothing to send, so this is a noop

        *bb = None;
    }

    /// Calculate how long a node should wait until it propagates a block. This is based on a relationship between the current difficulty and 
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
                blocks.push(self.ctx.rk.get_block_header(&p).map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into()))?);

                p = blocks[blocks.len() - 1].prev;

                if p == U256_ZERO {
                    blocks.pop(); // we do not want to include genesis itself
                    break; // we cannot go back any further, so also we have no need to continue.
                }
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

        Ok((U256::from_big_endian(&mut buf), max((max(validators.len(), 1) as f64).log(self.config.validators_count_base as f64).trunc() as u64, 1)))
    }

    /// Calculates the actual block difficulty, taking into account the current level of validators required and etc.
    fn calculate_expected_difficulty(&self, block: &Block) -> Result<u64, ForgeError> {
        let height = try!(self.ctx.rk.get_block_height(&block.header.prev)
            .map_err(|e| ForgeError(format!("Could not get a block height: {}", e).into()))) + 1;
        
        let base_diff = if height % self.config.recalculate_blocks != 0 {

            
            if height < self.config.recalculate_blocks {
                // use the starting difficulty
                Ok(1)
            }
            else {

                let block_data = bincode::deserialize::<EPoSBlockData>(
                    &try!(self.ctx.rk.get_block(&block.header.prev)
                        .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into()))).header.blob).unwrap();
                
                Ok(block_data.difficulty)
            }
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

            let pb = try!(self.ctx.rk.get_block(&hash_cur)
                    .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into())));
            
            for _ in 1..n {
                hash_cur = try!(self.ctx.rk.get_block(&hash_cur)
                    .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into()))).header.prev;
            }

            // how long *should* it have taken to get to this point?
            let expected = self.config.rate_target * n;

            let b = try!(self.ctx.rk.get_block(&hash_cur)
                    .map_err(|e| ForgeError(format!("Could not get a block from db: {}", e).into())));

            let actual = b.header.timestamp.diff(&pb.header.timestamp).millis() as u64;

            let last_diff = bincode::deserialize::<EPoSBlockData>(&pb.header.blob).unwrap().difficulty;

            debug!("Expected: {}, Actual: {}, Last Diff: {}", expected, actual, last_diff);

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

        // we have to check that the difficulty recorded in the block matches
        let res = bincode::deserialize::<EPoSBlockData>(&block.blob);
        if res.is_err() {
            warn!("Received block's EPoS data could not be decoded!");
            return false;
        }

        let cur_block_info = res.unwrap();

        if cur_block_info.difficulty != exp_diff {
            warn!("Received block's difficulty does not match ours!");
            return false;
        }

        // the last of the validation

        let mut cur_best: Option<(Block, u64)> = None;

        for &(ref key_id, ref key) in &self.keys {
            let mut key_block = block.clone();

            let res = EPoSBlockData::apply_block(&mut key_block, exp_diff, &target, &key);
            if let Err(e) = res {
                warn!("Block check was not valid: {:?}, {:?}", e, key_block);
                return false;
            }
            let actual = res.unwrap().get_relevant_validation_data().1;

            // get the stake of the account we are forging
            let res = self.ctx.rk.get_validator_stake(&key_id);
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
            let mut prev_best = self.ctx.best_block.lock().unwrap();

            if prev_best.is_some() {
                let pb = prev_best.as_mut().unwrap();
                if disp < pb.0 {
                    // update block and timeout
                    *pb = (req_validators, best.0, disp);

                    let ctx = Arc::clone(&self.ctx);

                    let thewait = Duration::from_millis(max(disp as i64 - Time::current().millis(), 10) as u64);

                    debug!("New best block candidate (wait {:?})", thewait);

                    self.ctx.remote.spawn(move |_| {

                        // this is guarenteed to be on the correct thread
                        let h = ctx.remote.handle().unwrap();

                        Timeout::new(thewait, &h)
                            .expect("Cannot start PoS propagate timer!")
                            .and_then(move |_| {
                                
                                EPoS::propagate_block(ctx);

                                Ok(())
                            })
                            .map_err(|_| ())
                        
                        //Ok(())
                    })
                }
            }
            else {
                *prev_best = Some((req_validators, best.0, disp));

                let thewait = Duration::from_millis(max(disp as i64 - Time::current().millis(), 10) as u64);

                debug!("New best block candidate (wait {:?})", thewait);

                let ctx = Arc::clone(&self.ctx);
                self.ctx.remote.spawn(move |_| {
                    // this is guarenteed to be on the correct thread
                    let h = ctx.remote.handle().unwrap();

                    Timeout::new(thewait, &h)
                        .expect("Cannot start PoS propagate timer!")
                        .and_then(move |_| {
                            
                            EPoS::propagate_block(ctx);

                            Ok(())
                        })
                        .map_err(|_| ())

                    //Ok(())
                })
            }
        }

        true
    }
}

impl BlockForger for EPoS {

    fn create(&self, block: Block) -> Box<Future<Item=Block, Error=ForgeError>> {
        let (tx, rx) = oneshot::channel();
        *self.ctx.on_block.lock().unwrap() = Some(tx);

        self.evaluate_block(block);
        Box::new(rx.map_err(|_| ForgeError(format!("Cancelled forge!"))))
    }

    fn validate(&self, block: &Block) -> Option<ForgeError> {

        // check that the difficulty matches what we expect
        let diff = self.calculate_expected_difficulty(block).expect("Database not working when validating block!");

        let res = self.calculate_validator_info(&block.prev);

        if let Err(e) = res {
            // TODO: Change logging strategy?
            return Some(ForgeError(format!("Forge validator calculation failed: {:?}", e)));
        }

        let (target, req_validators) = res.unwrap();

        if let Ok(b_data) = bincode::deserialize::<EPoSBlockData>(&block.header.blob) {
            if b_data.difficulty != diff {
                return Some(ForgeError(format!("Block difficulty is invalid")));
            }

            // check that there is the correct number of validator signatures
            if b_data.sigs.len() as u64 != req_validators {
                return Some(ForgeError(format!("Signature count does not match expected")));
            }

            // check that validators signatures are for real
            if !b_data.check_sigs(&target) {
                return Some(ForgeError(format!("Block signatures do not line up!")));
            }

            // check that the block timestamp checks out
            let mut wait = 0;

            for sig in b_data.sigs {
                let res = self.ctx.rk.get_validator_stake(&hash::hash_pub_key(&sig.0));
                if let Err(e) = res {
                    warn!("Could not get value held in account ({}): {:?}", hash::hash_pub_key(&sig.0), e);
                    return Some(ForgeError(format!("Failed to get a balance for validator: {}", hash::hash_pub_key(&sig.0))));
                }

                let stake = res.unwrap();

                wait += self.gen_wait(diff, stake, target.into(), sig.1.into());
            }

            // get the listed previous block, we should be timestamp + wait of that
            let res = self.ctx.rk.get_block_header(&block.prev).map_err(|e| ForgeError(format!("Could not get previous block: {:?}", e)));

            if res.is_err() {
                return res.err();
            }

            let prev_block = res.unwrap();

            if block.timestamp.millis() as u64 != prev_block.timestamp.millis() as u64 + wait {
                return Some(ForgeError(format!("Forging timestamps do not match")));
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

    /// Called when a broadcast is received. If the broadcast is to be propagated, the broadcast event must be re-called.
    /// Internally, network automatically handles duplicate events as a result of the reliable flood, so that can be safely ignored
    fn receive_broadcast(&self, _network_id: &U256, payload: &Vec<u8>) -> bool {
        if let Ok(block) = bincode::deserialize(&payload[..]) {
            self.evaluate_block(block);
        }

        false
    }
}

/// Verifies that block data is serialized, deserialized, generated, and verified properly in cycle
#[test]
fn block_data() {
    // generate two keys
    /*use openssl::rsa::Rsa;

    let k1 = Rsa::generate(2048).unwrap();
    let k2 = Rsa::generate(2048).unwrap();

    let epos = EPoS::new()*/
}

#[test]
fn calculate_difficulty() {

    use record_keeper::DummyRecordKeeper;
    use tokio_core::reactor::Core;

    let rk = Arc::new(DummyRecordKeeper::new());
    let rk2 = Arc::clone(&rk);

    let c = Core::new().unwrap();

    let mut epos_config = EPoSConfig::new(Vec::new());
    epos_config.recalculate_blocks = 20; // this speeds up the unit test dramatically

    let epos = EPoS::new(rk2, mpsc::unbounded().0, c.handle().remote().clone(), epos_config).unwrap();

    let b = rk.create_block().unwrap();
    assert_eq!(epos.calculate_expected_difficulty(&b).unwrap(), 1);

    // add blocks with a timestamp range which is 3x the base rate... aka 2 hours for 2160 blocks. We are moving forward an epoch
    for i in 0..epos.config.recalculate_blocks + 10 {
        let mut b = rk.create_block().unwrap();
        b.timestamp = Time::from_milliseconds((i * epos.config.rate_target / 3) as i64);

        let diff = epos.calculate_expected_difficulty(&b).expect("Should  be able to calculate difficulty from dummy RK");
        let block_data = EPoSBlockData {
            difficulty: diff,
            sigs: Vec::new()
        };

        b.blob = bincode::serialize(&block_data, bincode::Infinite).unwrap();
        rk.add_block(&b, true);
    }

    // now adding another block, the expected difficulty should be changed to 3
    let b = rk.create_block().unwrap();
    assert_eq!(epos.calculate_expected_difficulty(&b).unwrap(), 3);
}