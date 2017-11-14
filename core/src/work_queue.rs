use std::sync::{Arc, Weak, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::VecDeque;
use std::thread;
use primitives::{U256, Txn, Block, Event, EventListener, ListenerPool};
use record_keeper;
use record_keeper::{RecordKeeper};
use serde::{Serialize, Deserialize};
use time::Time;

/// An individual task to be completed by the `WorkQueue`.
#[derive(PartialEq, Eq)]
pub enum WorkItem {
    NewBlock(Block),
    NewTxn(Txn)
}
use self::WorkItem::*;

/// Work results define the completion status of a "finished" task. It is the result may be a
/// success message or it may be passing on the error it ran into.
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


/// The `WorkQueue` is designed as an overlay for the RecordKeeper (and possibly more) that allows
/// functions to be called on a separate thread from communication and incoming information which
/// creates the need for processing. As a result, it holds a listener pool which it will notify when
/// tasks are completed.
pub struct WorkQueue {
    rk: Arc<RecordKeeper>,
    queue: Mutex<VecDeque<WorkItem>>,
    listeners: Mutex<ListenerPool<WorkResult>>,
    run: AtomicBool
}

impl WorkQueue {
    pub fn new(rk: Arc<RecordKeeper>) -> WorkQueue {
        WorkQueue {
            rk,
            queue: Mutex::new(VecDeque::new()),
            listeners: Mutex::new(ListenerPool::new()),
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

    /// Add a reference to a listener which will be notified about completed work items.
    pub fn register_listener(&self, listener: Arc<EventListener<WorkResult>>) {
        self.listeners.lock().unwrap().register(&listener);
    }

    /// Returns true if the event was actually added, false if it is a duplicate and therefore not
    /// added to the queue. It will also return false if the queue is stopped.
    pub fn submit(&self, wi: WorkItem) -> bool {
        if !self.run.load(Ordering::Relaxed) { return false; }
        let mut queue = self.queue.lock().unwrap();
        if queue.contains(&wi) { false }
        else {queue.push_back(wi); true }
    }

    /// Primary running cycle to be called from a different thread. Will run until the flag `run` is
    /// marked as false.
    fn main_loop(&self) {
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
            self.listeners.lock().unwrap().notify(time, &result);
        }
    }

    /// Internal function to attempt adding a block to the system. Will return the work result.
    fn process_block(&self, block: Block) -> WorkResult {
        let hash = block.calculate_hash();
        match self.rk.add_block(&block) {
            Ok(true) => AddedNewBlock(hash),
            Ok(false) => DuplicateBlock(hash),
            Err(e) => ErrorAddingBlock(hash, e)
        }
    }

    /// Internal function to attempt adding a txn to the system. Will return the work result.
    fn process_txn(&self, txn: Txn) -> WorkResult {
        let hash = txn.calculate_hash();
        match self.rk.add_pending_txn(txn) {
            Ok(true) => AddedNewTxn(hash),
            Ok(false) => DuplicateTxn(hash),
            Err(e) => ErrorAddingTxn(hash, e)
        }
    }
}