use std::sync::{Arc, Weak, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::VecDeque;
use std::thread;
use primitives::{U256, Txn, Block, Event, EventListener};
use record_keeper;
use record_keeper::{RecordKeeper};
use serde::{Serialize, Deserialize};
use time::Time;

#[derive(PartialEq, Eq)]
pub enum WorkItem {
    NewBlock(Block),
    NewTxn(Txn)
}
use self::WorkItem::*;

#[derive(Clone, Debug)]
pub enum WorkResult {
    AddedNewBlock(U256),
    DuplicateBlock(U256),
    ErrorAddingBlock(U256, record_keeper::Error),
    
    AddedNewTxn(U256),
    DuplicateTxn(U256),
    ErrorAddingTxn(U256, record_keeper::Error)
}
impl Event for WorkResult {}
use self::WorkResult::*;

/// Work Result Listener
type WRL = EventListener<WorkResult>;



pub struct WorkQueue {
    rk: Arc<RecordKeeper>,
    queue: Mutex<VecDeque<WorkItem>>,
    listener: Weak<WRL>,
    run: AtomicBool
}

impl WorkQueue {
    pub fn new(rk: Arc<RecordKeeper>, listener: &Arc<WRL>) -> WorkQueue {
        WorkQueue {
            rk,
            queue: Mutex::new(VecDeque::new()),
            listener: Arc::downgrade(listener),
            run: AtomicBool::new(false)
        }
    }

    pub fn start(queue: &Arc<WorkQueue>) -> thread::JoinHandle<()> {
        let mut q = Arc::clone(queue);
        thread::spawn(move || q.main_loop())
    }

    /// Prevent new items from being added and stop the running thread as soon as it has completed
    /// its current task.
    pub fn stop(&self) -> bool {
        self.run.swap(false, Ordering::Relaxed)
    }

    /// Returns true if the event was actually added, false if it is a duplicate and therefore not
    /// added to the queue. It will also return false if the queue is stopped.
    pub fn add_event(&self, wi: WorkItem) -> bool {
        if !self.run.load(Ordering::Relaxed) { return false; }
        let mut queue = self.queue.lock().unwrap();
        if queue.contains(&wi) { false }
        else {queue.push_back(wi); true }
    }

    /// Primary running cycle to be called from a different thread. Will run until the flag `run` is
    /// marked as false.
    fn main_loop(&self) {
        let listener =
            if let Some(l) = self.listener.upgrade() { l }
            else { self.run.store(false, Ordering::Relaxed); return; };

        while self.run.load(Ordering::Relaxed) {
            let task = {
                let mut queue = self.queue.lock().unwrap();
                queue.pop_front()
            };

            if task.is_none() {
                thread::yield_now();
                continue;
            }
            
            let result = match task.unwrap() {
                NewBlock(block) => self.process_block(block),
                NewTxn(txn) => self.process_txn(txn)
            };
            let time = Time::current().millis() as u64;
            listener.notify(time, result);
        }
    }

    fn process_block(&self, block: Block) -> WorkResult {
        let hash = block.calculate_hash();
        match self.rk.add_block(&block) {
            Ok(true) => AddedNewBlock(hash),
            Ok(false) => DuplicateBlock(hash),
            Err(e) => ErrorAddingBlock(hash, e)
        }
    }

    fn process_txn(&self, txn: Txn) -> WorkResult {
        let hash = txn.calculate_hash();
        match self.rk.add_pending_txn(txn) {
            Ok(true) => AddedNewTxn(hash),
            Ok(false) => DuplicateTxn(hash),
            Err(e) => ErrorAddingTxn(hash, e)
        }
    }
}