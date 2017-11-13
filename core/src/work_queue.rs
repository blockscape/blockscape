use std::sync::{Arc, Weak, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::VecDeque;
use std::thread;
use primitives::{U256, Txn, Block, Event, EventListener};
use record_keeper;
use record_keeper::{RecordKeeper};
use serde::{Serialize, Deserialize};

#[derive(PartialEq, Eq)]
pub enum WorkItem {
    NewBlock(Block),
    NewTxn(Txn)
}

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



pub struct WorkQueue {
    rk: Arc<RecordKeeper>,
    queue: Mutex<VecDeque<WorkItem>>,
    listener: Weak<EventListener<WorkResult>>,
    run: AtomicBool
}

impl WorkQueue {
    pub fn new(rk: Arc<RecordKeeper>, listener: &Arc<EventListener<WorkResult>>) -> WorkQueue {
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
            
            match task.unwrap() {
                WorkItem::NewBlock(Block) => {
                    
                },
                WorkItem::NewTxn(Txn) => {

                }
            }
        }
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
}