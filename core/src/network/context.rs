use std::borrow::Borrow;
use std::cell::*;
use std::sync::Arc;
use std::io;
use std::rc::Rc;
use std::collections::HashMap;

use futures::prelude::*;
use futures::stream;
use futures::sink::BoxSink;

use tokio_core::reactor::*;

use record_keeper::RecordKeeper;

use primitives::U256;
use time::Time;
use hash::hash_bytes;
use env::get_client_name;

use network::protocol::*;
use network::client::{ClientConfig, BroadcastReceiver};
use network::node::{Node,NodeEndpoint,Protocol};
use network::shard::*;
use network::node::*;

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

    /// List of received broadcast hashes
    pub received_broadcasts: RefCell<HashMap<U256, Time>>,

    /// Receivers which are registered to receive events; any payloads not fitting to this list will be dropped.
    pub broadcast_receivers: [Cell<Option<Arc<BroadcastReceiver + Send + Sync>>>; 256],


    /// Data structures associated with shard-specific information
    shards: [RefCell<Option<ShardInfo>>; 255],

    /// dummy empty shard, basically required because rust cannot handle RefCell statics and refs very well
    empty_shard: RefCell<Option<ShardInfo>>,

    curr_port: Cell<u8>,

    num_shards: Cell<u8>,
}

impl NetworkContext {

    pub fn new(config: ClientConfig, rk: Arc<RecordKeeper>, core: &Core) -> NetworkContext {
        
        let pkeyder = config.private_key.public_key_to_der().expect("Could not convert node key to der!");
        let epoint = NodeEndpoint {
            host: config.hostname.clone(),
            port: config.port,
            protocol: Protocol::Udp
        };
        
        NetworkContext {
            rk: rk,
            config: config,
            my_node: Node {
                key: pkeyder, // TODO: Should be public key only!
                version: PROTOCOL_VERSION,
                endpoint: epoint,
                name: get_client_name()
            },
            event_loop: core.handle(), 
            sink: Cell::new(None),
            received_broadcasts: RefCell::new(HashMap::new()),
            broadcast_receivers: init_array!(Cell<Option<Arc<BroadcastReceiver + Send + Sync>>>, 256, Cell::new(None)),
                
            shards: init_array!(RefCell<Option<ShardInfo>>, 255, RefCell::new(None)),
            empty_shard: RefCell::new(None),
            num_shards: Cell::new(0),
            curr_port: Cell::new(0),
        }
    }

    #[inline]
    pub fn udp_send_packets(&self, p: Vec<SocketPacket>) {
        if !p.is_empty() {
            let st = stream::iter_ok::<_, io::Error>(p);
            // TODO: Try to eliminate call to wait! Typically it should not be an issue, but
            // it would be more future-ist to provide some way to react upon future availability
            self.sink.set(Some(st.forward(self.sink.replace(None).unwrap()).wait().unwrap().1));
        }
    }

    /// Forwards the received broadcast to the appropriate handler, or returns false if the handler does not exist or if the hash has alraedy been received
    pub fn handle_broadcast(&self, network_id: &U256, id: u8, payload: &Vec<u8>) -> bool {
        let incoming_hash = hash_bytes(&payload[..]);

        if self.received_broadcasts.borrow().contains_key(&incoming_hash) {
            // should not be propogating broadcasts multiple times
            return false
        }

        if let Some(receiver) = self.broadcast_receivers[id as usize].replace(None) {
            self.received_broadcasts.borrow_mut().insert(incoming_hash, Time::current_local());
            let r = receiver.receive_broadcast(network_id, payload);

            self.broadcast_receivers[id as usize].replace(Some(receiver));

            r
        }
        else {
            false
        }
    }

    /// Initialize a node repository from file given the ID
    /// NOTE: This is pretty slow, consider using sparingly
    fn load_node_repo(&self, network_id: U256) -> NodeRepository {
        let mut repo = NodeRepository::new();

        let res = repo.load(network_id.to_string().as_str());
        
        if res.is_ok() && res.unwrap() == 0 {
            // add seed nodes
            repo.build(&self.config.seed_nodes.iter().cloned().map(|ep| LocalNode::new(Node::new(ep))).collect());
        }

        repo
    }

    /// Connect to the specified shard by shard ID. On success, returns the number of pending connections (the number of nodes)
    /// A result value of 0 does not indicate failure; it simply means that we need some time to gain connections within the net.
    /// Be patient.
    pub fn attach_network(this: &Rc<NetworkContext>, network_id: U256, mode: ShardMode) -> Result<usize, ()> {

        if this.num_shards.get() > 128 {
            // we risk overwhelming the ports
            return Err(());
        }

        // first, setup the node repository
        let repo = this.load_node_repo(network_id);
        let node_count = repo.len();

        debug!("Attached network repo size: {}", node_count);

        // find a suitable port
        let mut port;
        loop {
            port = (this.curr_port.replace((this.curr_port.get() + 1) % 255)) as u8;

            // make sure the port is not taken (this should almost always take one try)
            if this.shards[port as usize].borrow().is_none() {
                break;
            }
        }

        // we can now get going
        let si = ShardInfo::new(network_id, port, mode, Rc::clone(&this), repo);

        let mut shard = this.shards[port as usize].borrow_mut();
        *shard = Some(si);

        // TODO: Constant?
        if node_count >= 2 {
            // we can start connecting to nodes immediately
            let r = shard.borrow().as_ref().unwrap().node_scan(8);
            Ok(r)
        }
        else {
            // we need to resolve our way over to this shard (TODO)
            Ok(0)
        }
    }

    pub fn detach_network(&self, network_id: &U256) -> bool {
        self.detach_network_port(self.resolve_port(network_id))
    }

    fn detach_network_port(&self, idx: u8) -> bool {
        debug!("Detach network port: {}", idx);

        let mut sh = self.shards[idx as usize].borrow_mut();
        if let None = *sh {
            return false;
        }

        sh.as_ref().unwrap().close();
        *sh = None;

        true
    }

    pub fn resolve_port(&self, network_id: &U256) -> u8 {
        for i in 0..255 {
            let shard = self.shards[i].borrow();
            if let Some(ref sh) = *shard {
                if sh.get_network_id() == network_id {
                    return i as u8;
                }
            }
        }

        255
    }

    pub fn get_shard(&self, port: u8) -> Ref<Option<ShardInfo>> {
        self.shards[port as usize].borrow()
    }

    pub fn get_shard_by_id(&self, network_id: &U256) -> Ref<Option<ShardInfo>> {
        let p = self.resolve_port(network_id);

        if p < 255 {
            self.get_shard(p)
        }
        else {

            self.empty_shard.borrow()
        }
    }

    /// End all network resources and prepare for program close
    /// You are still responsible for joining to the network threads to make sure they close properly
    pub fn close(&self) {

        debug!("Closing network...");

        // detach all networks
        for i in 0..255 {
            let exists = self.shards[i].borrow().is_some();

            if exists {
                self.detach_network_port(i as u8);
            }
        }
    }
}
