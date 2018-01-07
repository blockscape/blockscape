use std::cell::Cell;
use std::sync::Arc;
use std::io;

use futures::prelude::*;
use futures::stream;
use futures::sink::BoxSink;

use tokio_core::reactor::*;

use record_keeper::RecordKeeper;

use network::session::SocketPacket;
use network::client::ClientConfig;
use network::node::Node;

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