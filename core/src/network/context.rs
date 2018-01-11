use std::cell::Cell;
use std::sync::Arc;
use std::io;
use std::rc::Rc;

use futures::prelude::*;
use futures::stream;
use futures::sink::BoxSink;
use futures::unsync;

use tokio_core::reactor::*;

use record_keeper::RecordKeeper;

use primitives::U256;

use network::session::SocketPacket;
use network::client::ClientConfig;
use network::node::Node;

/// The amount of data a job is allowed to allocate in memory before it is 
pub const MAX_JOB_SIZE: usize = 100 * 1024 * 1024;

#[derive(Clone)]
pub struct NetworkJob {
    /// The network this job is resolving for
    pub network_id: U256,

    /// The block hash we are trying to reach
    target: Cell<U256>,

    /// The hash of the block we are expected to be on when concurrent returns to 0 (AKA all current imports are done)
    pub predicted_cur: Cell<U256>,

    /// The number of currently pending requests/import processes
    pub concurrent: Cell<usize>,

    /// The number of times this job has failed to resolve
    pub try: Cell<usize>,

    /// The job to process after accomplishing this job
    pub then: Option<Rc<NetworkJob>>
}

impl NetworkJob {
    pub fn new(network_id: U256, target: U256, cur: U256, then: Option<Rc<NetworkJob>>) -> Rc<NetworkJob> {
        Rc::new(NetworkJob {
            network_id,
            target: Cell::new(target),
            predicted_cur: Cell::new(cur),
            concurrent: Cell::new(0),
            try: Cell::new(0),
            then
        })
    }

    pub fn get_target(&self) -> U256 {
        self.target.get()
    }

    // called when a new hash has been discovered which references this current previous one, meaning this job should be updated
    pub fn augment(&self, new_target: U256) {
        self.target.set(new_target);
    }
}

pub struct NetworkContext {
    /// Access to the backend database/management engine
    pub rk: Arc<RecordKeeper>,

    /// The event loop for the network handling thread
    pub event_loop: Handle,
    
    /// A future which leads to the sink which can be used to send more packets.
    /// Note that the option here is only a dummy: it is set to none while the value is being swapped only,
    /// so it should always be Some for the usecase of running a sink.
    pub sink: Cell<Option<BoxSink<SocketPacket, io::Error>>>,


    /// Configuration options for the behavior of the network client
    pub config: ClientConfig,

    /// The node object which represents my own system
    pub my_node: Node,

    /// A place to chain data which should be retrieved. The second value in the tuple, a hash, is used to
    /// identify a possible augmentation. In this case, it is always the previous
    pub job_targets: unsync::mpsc::UnboundedSender<(Rc<NetworkJob>, Option<U256>)>,
}

impl NetworkContext {
    #[inline]
    pub fn send_packets(&self, p: Vec<SocketPacket>) {
        if !p.is_empty() {
            let st = stream::iter_ok::<_, io::Error>(p);
            // TODO: Try to eliminate call to wait! Typically it should not be an issue, but
            // it would be more future-ist to provide some way to react upon future availability
            self.sink.set(Some(st.forward(self.sink.replace(None).unwrap()).wait().unwrap().1));
        }
    }
}