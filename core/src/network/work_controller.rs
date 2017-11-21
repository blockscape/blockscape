use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use std::sync::{Arc, Mutex};
use std::any::Any;

use primitives::EventListener;

use primitives::u160::*;
use primitives::u256::*;

use network::session::{Packet, Message};

use record_keeper::RecordKeeper;
use record_keeper::error::Error;
use record_keeper::database::BLOCKCHAIN_POSTFIX;

use primitives::{Block, Txn};

use bincode::deserialize;

use work_queue::*;

// TODO: Work request is currently a little expensive in terms of memory usage
struct NetworkWorkRequest {
    /// A wrapping identifier to track batch processing progression
    batch: u32,
    /// The specific index of the item processed in the batch, numbered from max-0
    item: u32,
    /// The node responsible for sending this data/job
    provider: U160,
}

struct WorkTarget {
    target: U256,

    /// For bogus detection, if a hash is not found after too many tries, it is rejected
    pub failures: u8,
}

impl WorkTarget {
    pub fn new(hash: U256) -> WorkTarget {
        WorkTarget {
            target: hash,
            failures: 0
        }
    }

    pub fn get_target(&self) -> &U256 {
        &self.target
    }
}

impl Hash for WorkTarget {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
    }
}

impl PartialEq for WorkTarget {
    fn eq(&self, other: &WorkTarget) -> bool {
        self.target == other.target
    }
}

impl Eq for WorkTarget {}

struct NetworkWorkControllerData {
    /// The record keeper/database
    rk: Arc<RecordKeeper>,

    /// Reference a `WorkQueue` which can be tasked with the heavy lifting and calling the record keeper.
    work_queue: Arc<WorkQueue>,

    /// The object hashes we are attempting to sync. If this list is empty, no querying is necessary
    /// * __Note__: May include a special 0x000... hash to signify generic syncing while a hash is not known
    targets: HashSet<WorkTarget>,

    /// A mapping of active batches to targets
    active_batches: HashMap<u32, U256>,

    /// The most recently submitted batch ID
    last_batch: u32,

    send_ring: Vec<Packet>
}

/// Organizes and queues up work efficiently from the network for WorkQueue
pub struct NetworkWorkController(Mutex<NetworkWorkControllerData>);

impl NetworkWorkController {
    pub fn new(rk: Arc<RecordKeeper>, wq: Arc<WorkQueue>) -> Arc<NetworkWorkController> {
        let mut cont = NetworkWorkController(Mutex::new(NetworkWorkControllerData {
            rk: rk,
            work_queue: wq.clone(),
            targets: HashSet::new(),
            active_batches: HashMap::new(),
            last_batch: 0,
            send_ring: Vec::new()
        }));

        cont.0.lock().unwrap().targets.insert(WorkTarget::new(U256_ZERO));

        let a = Arc::new(cont);

        /// register itself to the work queue
        wq.register_listener(Arc::clone(&a) as Arc<EventListener<WorkResult>>);

        a
    }

    /// Process a block which has been received from the network. Return whether or not it would make sense
    /// to retransmit due to sufficient validation.
    pub fn import_block(&self, block: &Block) -> bool {
        let h = block.calculate_hash();

        let mut s = self.0.lock().unwrap();

        if s.targets.contains(&WorkTarget::new(h)) {
            return false;
        }

        let prev_target = WorkTarget::new(block.prev);
        if s.targets.contains(&prev_target) {
            // augment our current target
            let mut wt = WorkTarget::new(h);

            s.targets.remove(&prev_target);
            s.targets.insert(wt);

            for v in s.active_batches.values_mut() {
                if v == prev_target.get_target() {
                    *v = h;
                }
            }

            return false;
        }

        // try to attach to the chain
        let res = s.rk.add_block(&block);
        if let Ok(added) = res {
            added
        }
        else {
            let err = res.unwrap_err();
            if let Error::NotFound(prefix, hash) = err {
                if prefix != BLOCKCHAIN_POSTFIX {
                    // TODO: Possibly panic
                    return false;
                }

                // need to resolvetxns.len() + 
                s.targets.insert(WorkTarget::new(h));
 
                s.last_batch += 1; 

                let seq = s.last_batch;
                let cur_block_hash = s.rk.get_current_block_hash();

                s.send_ring.push(Packet {
                    seq: seq,
                    msg: Message::SyncBlocks {
                        last_block_hash: cur_block_hash,
                        target_block_hash: h
                    }
                });
            }
            else if let Error::Logic(err) = err {
                // TODO: Figure out if action needs to be taken to the submitting node
            }

            false
        }
    }

    pub fn import_txn(&self, txn: &Txn) -> bool {
        // try to attach to the chain

        let s = self.0.lock().unwrap();

        let res = s.rk.add_pending_txn(txn);
        if let Ok(added) = res {
            added
        }
        else {
            let err = res.unwrap_err();
            if let Error::NotFound(prefix, hash) = err {
                if prefix != BLOCKCHAIN_POSTFIX {
                    // TODO: Possibly panic
                    return false;
                }

                // TODO: need to resolve
                /*self.targets.insert(WorkTarget::new(h));
 
                // spot request
                self.last_batch += 1;                
                self.send_ring.push(Packet {
                    seq: last_batch,
                    msg: Message::SyncBlocks {
                        last_block_hash: self.rk.get_current_block_hash(),
                        target_block_hash: h
                    }
                });*/
            }
            else if let Error::Logic(err) = err {
                // TODO: Figure out if action needs to be taken to the submitting node
            }

            false
        }
    }

    pub fn import_bulk(&self, seq: u32, provider: &U160, mut blocks: Vec<Block>, mut txns: Vec<Txn>) {

        let mut s = self.0.lock().unwrap();

        while let Some(txn) = txns.pop() {
            s.work_queue.submit(WorkItem(
                Task::NewTxn(txn),
                Some(Box::new(NetworkWorkRequest {
                    batch: seq,
                    item: txns.len() as u32 + blocks.len() as u32,
                    provider: provider.clone()
                }))
            ));
        }

        while let Some(block) = blocks.pop() {
            s.work_queue.submit(WorkItem(
                Task::NewBlock(block),
                Some(Box::new(NetworkWorkRequest {
                    batch: seq,
                    item: blocks.len() as u32,
                    provider: provider.clone()
                }))
            ));
        }
    }
}

impl EventListener<WorkResult> for NetworkWorkController {
    /// Add an event to the queue of finished things such that it can be handled by the main loop at
    /// a later point.
    fn notify(&self, time: u64, r: &WorkResult) {
        use self::WorkResultType::*;

        let mut s = self.0.lock().unwrap();

        let &WorkResult(ref result, ref meta) = r; //TODO: use metadata

        /*if let tag = meta.map(|m| (m as Box<Any>).downcast_ref::<NetworkWorkRequest>()).unwrap_or(None) {
            
        }*/

        if meta.is_some() {
            let tmp = &meta.unwrap() as &Any;

            let tagr = tmp.downcast_ref::<NetworkWorkRequest>();

            if let Some(ref tag) = tagr {
                match result { //TODO: fill these out with the appropriate responses
                    &AddedNewBlock(ref hash) => {
                        if tag.item == 0 {
                            // request the next batch
                            if let Some(targeth) = s.active_batches.get(&tag.batch) {
                                s.last_batch += 1;
                                
                                s.send_ring.push(Packet {
                                    seq: s.last_batch,
                                    msg: Message::SyncBlocks {
                                        last_block_hash: hash.clone(),
                                        target_block_hash: targeth.clone()
                                    }
                                });
                            }
                        }
                    },
                    &DuplicateBlock(ref hash) => {
                        // here we treat it like success all the same
                        if tag.item == 0 {
                            // request the next batch
                            if let Some(targeth) = s.active_batches.get(&tag.batch) {
                                s.last_batch += 1;
                                
                                s.send_ring.push(Packet {
                                    seq: s.last_batch,
                                    msg: Message::SyncBlocks {
                                        last_block_hash: hash.clone(),
                                        target_block_hash: targeth.clone()
                                    }
                                });
                            }
                        }
                    },
                    &ErrorAddingBlock(ref hash, ref e) => {
                        // add new targets?
                        if let &Error::NotFound(ref prefix, ref hash) = e {
                            if prefix != &BLOCKCHAIN_POSTFIX {
                                // TODO: Possibly panic
                                return;
                            }

                            s.targets.insert(WorkTarget::new(deserialize(&hash).unwrap()));
                        }
                        else if let &Error::Logic(ref err) = e {
                            // TODO: Figure out if action needs to be taken to the submitting node
                        }
                    },
                    &AddedNewTxn(ref hash) => {
                        // right now do not care except error
                    },
                    &DuplicateTxn(ref hash) => {
                        // right now do not care except error
                    },
                    &ErrorAddingTxn(ref hash, ref e) => {
                        // add new targets?
                        if let &Error::NotFound(ref prefix, ref hash) = e {
                            if prefix != &BLOCKCHAIN_POSTFIX {
                                // TODO: Possibly panic
                                return;
                            }

                            s.targets.insert(WorkTarget::new(deserialize(&hash).unwrap()));
                        }
                        else if let &Error::Logic(ref err) = e {
                            // TODO: Figure out if action needs to be taken to the submitting node
                        }
                    }
                }
            }
        }
    }
}