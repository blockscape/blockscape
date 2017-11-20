use primitives::{U256, Txn, Block, Event, EventListener, ListenerPool};
use record_keeper;
use record_keeper::{RecordKeeper};
use std::any::Any;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use time::Time;
use std::fmt;

/// An individual task to be completed by the `WorkQueue`.
#[derive(PartialEq, Eq)]
pub enum Task {
    NewBlock(Block),
    NewTxn(Txn)
}
use self::Task::*;

pub type MetaData = Option<Box<Any + Send + Sync + 'static>>;

/// A task tagged with some meta data which will be returned when the task is completed. Note that
/// the meta data is only passed on by the work queue and is not used in processing.
pub struct WorkItem(pub Task, pub MetaData);
impl PartialEq for WorkItem {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
} impl Eq for WorkItem {}

/// Work results define the completion status of a "finished" task. It is the result may be a
/// success message or it may be passing on the error it ran into.
#[derive(Debug)]
pub enum WorkResultType {
    AddedNewBlock(U256),
    DuplicateBlock(U256),
    ErrorAddingBlock(U256, record_keeper::Error),
    
    AddedNewTxn(U256),
    DuplicateTxn(U256),
    ErrorAddingTxn(U256, record_keeper::Error)
}
use self::WorkResultType::*;

/// Includes the type of work result and also any meta data which was attached to the task,
pub struct WorkResult(pub WorkResultType, pub MetaData);
impl Event for WorkResult {}
impl fmt::Debug for WorkResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

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
        let q = Arc::clone(queue);
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
            let next = {
                let mut queue = self.queue.lock().unwrap();
                queue.pop_front()
            };

            if next.is_none() {
                thread::yield_now();
                continue;
            }

            let WorkItem(task, metadata) = next.unwrap();
            let result = WorkResult( match task {
                NewBlock(block) => self.process_block(block),
                NewTxn(txn) => self.process_txn(txn)
            }, metadata);

            let time = Time::current().millis() as u64;
            self.listeners.lock().unwrap().notify(time, &result);
        }
    }

    /// Internal function to attempt adding a block to the system. Will return the work result.
    fn process_block(&self, block: Block) -> WorkResultType {
        let hash = block.calculate_hash();
        match self.rk.add_block(&block) {
            Ok(true) => AddedNewBlock(hash),
            Ok(false) => DuplicateBlock(hash),
            Err(e) => ErrorAddingBlock(hash, e)
        }
    }

    /// Internal function to attempt adding a txn to the system. Will return the work result.
    fn process_txn(&self, txn: Txn) -> WorkResultType {
        let hash = txn.calculate_hash();
        match self.rk.add_pending_txn(&txn) {
            Ok(true) => AddedNewTxn(hash),
            Ok(false) => DuplicateTxn(hash),
            Err(e) => ErrorAddingTxn(hash, e)
        }
    }
}