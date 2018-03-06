use bincode;
use openssl::pkey::PKey;
use std::cell::*;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::rc::*;
use std::borrow::Borrow;
use std::thread;
use std::time::Duration;

use futures::prelude::*;
use futures::sync::*;
use futures::sync::mpsc::{UnboundedSender, unbounded};
use futures::future;
use futures::unsync;

use tokio_core::reactor::*;
use tokio_core::net::{UdpSocket, UdpCodec};

use env::get_client_name;
use network::context::*;
use network::node::{Node, NodeRepository, NodeEndpoint, LocalNode};
//use network::ntp;
use network::session;
use network::session::{SessionInfo, SocketPacket, Message};
use network::shard::{ShardInfo};
use primitives::U256;
use record_keeper::{RecordKeeper, RecordEvent};
use signer::generate_private_key;
use util::QuitSignal;

/// Defines the kind of interaction this node will take with a particular shard
pub enum ShardMode {
    /// Full participation, operating in block mining, full work processing, full authority
    Primary,
    /// This is a long term connection and we still validate and sync on this shard, but less processing, primarily just validation
    Auxillery,
    /// Used when connecting to a shard to only get information from authoritative network sources. Good for when a player views a arbitrary shard
    /// for gameplay. 
    QueryOnly
}

//const NODE_SCAN_INTERVAL: u64 = 30000; // every 30 seconds
const NODE_CHECK_INTERVAL: u64 = 5000; // every 5 seconds
//const NODE_NTP_INTERVAL: u64 = 20 * 60000; // every 20 minutes

/// The maximum amount of data that can be in a single message object (the object itself can still be in split into pieces at the datagram level)
/// Right now it is set to 64k, which is the highest number supported by kernel fragmentation right now.
pub const MAX_PACKET_SIZE: usize = 64 * 1000;

//#[derive(Debug)]
pub struct ClientConfig {
    /// Hostname to advertise as the node address, useful for DNS round robin or load balancing if wanted
    pub hostname: String,

    /// The port to listen for UDP packets on and bind to
    pub port: u16,

    /// Sets a threshold which, at sufficiently low connectivity of nodes (AKA, less than this number), new nodes will be seeked out
    pub min_nodes: u16,

    /// Sets the maximum simultaneous node connections
    pub max_nodes: u16,

    /// Synchronization servers for calculating time offset
    pub ntp_servers: Vec<String>,

    /// Endpoints to connect for a network initially if no node is available to connect to
    pub seed_nodes: Vec<NodeEndpoint>,

    /// The address used for listening (for open)
    pub bind_addr: SocketAddr,

    /// A private key used to sign and identify our own node data
    pub private_key: PKey
}

impl ClientConfig {

    /// Reccomended communication port for P2P blockscape protocol
    pub const DEFAULT_PORT: u16 = 35653;

    /// Initializes the config with reasonable defaults
    pub fn new() -> ClientConfig {
        ClientConfig::from_key(generate_private_key())
    }

    pub fn from_key(key: PKey) -> ClientConfig {
        ClientConfig {
            private_key: key,
            ntp_servers: vec!["pool.ntp.org".into()],
            seed_nodes: vec![
                NodeEndpoint {
                    host: String::from("seed-1.blockscape"),
                    port: 35653
                },
                NodeEndpoint {
                    host: String::from("seed-2.blockscape"),
                    port: 35653
                }
            ],
            min_nodes: 8,
            max_nodes: 16,
            hostname: String::from(""),
            port: ClientConfig::DEFAULT_PORT,
            bind_addr: SocketAddr::new("0.0.0.0".parse().unwrap(), ClientConfig::DEFAULT_PORT)
        }
    }
}

pub enum ClientMsg {
    GetStatistics(oneshot::Sender<Statistics>),
    GetPeerInfo(oneshot::Sender<Vec<SessionInfo>>),
    AddNode(U256, Node),
    DropNode(U256, Node),

    AttachNetwork(U256, ShardMode),
    DetachNetwork(U256),

    ShouldForge(U256, oneshot::Sender<bool>)
}

/// Statistical information which can be queried from the network client
#[derive(Debug, Serialize, Deserialize)]
pub struct Statistics {
    /// The number of networks currently registered/working on this node
    pub attached_networks: u8,

    /// Thu number of nodes currently connected
    pub connected_peers: u32,

    /// Number of bytes received since the client started execution
    pub rx: u64,

    /// Number of bytes sent since the client started execution
    pub tx: u64,

    /// Number of milliseconds of average latency between peers
    pub avg_latency: u64
}

impl Statistics {
    fn new() -> Statistics {
        Statistics {
            attached_networks: 0,
            connected_peers: 0,
            rx: 0,
            tx: 0,
            avg_latency: 0
        }
    }
}

/// A data structure which is passed into session handlers while packets are being processed and during certain events.
/// The inclusion of this data allows for reactions to be taken by core as appropriate
pub struct NetworkActions<'a> {

    /// The network client instance for certain generative calls
    pub nc: &'a Client,

    /// Nodes which can be connected to which were recently supplied
    pub connect_peers: HashMap<U256, Vec<Node>>,

    /// Packets which should be sent
    pub send_packets: Vec<SocketPacket>
}

impl<'a> NetworkActions<'a> {
    pub fn new(nc: &'a Client) -> NetworkActions<'a> {
        NetworkActions {
            nc: nc,
            connect_peers: HashMap::new(),
            send_packets: Vec::new()
        }
    }
}

struct P2PCodec;

impl UdpCodec for P2PCodec {
    type In = SocketPacket;
    type Out = SocketPacket;

    fn decode(&mut self, src: &SocketAddr, buf: &[u8]) -> io::Result<Self::In> {
        Ok(SocketPacket(src.clone(), bincode::deserialize(buf).map_err(|_| io::ErrorKind::Other)?))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> SocketAddr {
        buf.extend(bincode::serialize(&msg.1, bincode::Infinite).unwrap());

        msg.0
    }
}

pub struct Client {

    /// Shared data for all network building blocks
    context: Rc<NetworkContext>,

    /// Data structures associated with shard-specific information
    shards: [RefCell<Option<ShardInfo>>; 255],

    /// All the currently active jobs are kept here. For augmentation.
    jobs: RefCell<Vec<Weak<NetworkJob>>>,

    curr_port: Cell<u8>,

    num_shards: Cell<u8>
}

impl Client {
    fn new(config: ClientConfig, rk: Arc<RecordKeeper>, core: &Core, chain_tx: unsync::mpsc::UnboundedSender<(Rc<NetworkJob>, Option<U256>)>) -> Client {
        let pkeyder = config.private_key.public_key_to_der().expect("Could not convert node key to der!");
        let epoint = NodeEndpoint { host: config.hostname.clone(), port: config.port };
        
        Client {
            context: Rc::new(NetworkContext {
                rk: Arc::clone(&rk),
                config: config,
                my_node: Node {
                    key: pkeyder, // TODO: Should be public key only!
                    version: session::PROTOCOL_VERSION,
                    endpoint: epoint,
                    name: get_client_name()
                },
                event_loop: core.handle(), 
                sink: Cell::new(None),
                job_targets: chain_tx,
            }),
            shards: init_array!(RefCell<Option<ShardInfo>>, 255, RefCell::new(None)),
            jobs: RefCell::new(Vec::new()),
            num_shards: Cell::new(0),
            curr_port: Cell::new(0)
        }
    }

    /// Connect to the specified shard by shard ID. On success, returns the number of pending connections (the number of nodes)
    /// A result value of 0 does not indicate failure; it simply means that we need some time to gain connections within the net.
    /// Be patient.
    pub fn attach_network(&self, network_id: U256, mode: ShardMode) -> Result<usize, ()> {

        if self.num_shards.get() > 128 {
            // we risk overwhelming the ports
            return Err(());
        }

        // first, setup the node repository
        let repo = self.load_node_repo(network_id);
        let node_count = repo.len();

        debug!("Attached network repo size: {}", node_count);

        // find a suitable port
        let mut port;
        loop {
            port = (self.curr_port.replace((self.curr_port.get() + 1) % 255)) as u8;

            // make sure the port is not taken (this should almost always take one try)
            if self.shards[port as usize].borrow().is_none() {
                break;
            }
        }

        // we can now get going
        let si = ShardInfo::new(network_id, port, mode, Rc::clone(&self.context), repo);

        let mut shard = self.shards[port as usize].borrow_mut();
        *shard = Some(si);

        // TODO: Constant?
        if node_count >= 2 {
            // we can start connecting to nodes immediately
            let mut actions = NetworkActions::new(self);
            let r = shard.borrow().as_ref().unwrap().node_scan(8, &mut actions);

            self.context.send_packets(actions.send_packets);
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

        let mut actions = NetworkActions::new(self);

        sh.as_ref().unwrap().close(&mut actions);
        *sh = None;

        true
    }

    fn resolve_port(&self, network_id: &U256) -> u8 {
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

    fn process_packet(&self, d: SocketPacket) -> Box<Future<Item=(), Error=io::Error>> {
        // send packet to the correct shard
        let SocketPacket(addr, p) = d;
        
        let mut actions = NetworkActions::new(self);

        if p.port == 255 {
            if let Message::Introduce { ref node, ref network_id, .. } = p.payload.msg {
                // new session?
                let idx = self.resolve_port(network_id);
                if let Some(ref shard) = *self.shards[idx as usize].borrow() {
                    if let Ok(addr) = shard.open_session(Arc::new(node.clone()), Some(&p.payload), &mut actions) {
                        info!("New contact opened from {}", addr);


                    }
                }
                else {
                    debug!("Invalid network ID received in join for network: {}", network_id);
                }
            }
            else {
                debug!("Received non-introduce first packet on generic port: {:?}", p);
            }
        }
        else if let Some(ref shard) = *self.shards[p.port as usize].borrow() {

            shard.process_packet(&p.payload, &addr, &mut actions);
        }
        else {
            // bogus network ID received, ignore
            // TODO: A good debug print here might also print the packet
            debug!("Received unregistered network port packet: {}", p.port);
        }

        // finally, any new nodes to connect to?
        for (network_id, peers) in actions.connect_peers.iter() {
            for peer in peers {
                // for right now, only save for nodes of open networks
                let port = self.resolve_port(network_id) as usize;
                if port != 255 {
                    // add to the connect queue of the network
                    // a successful connection will result in the node being added to the permanent db
                    if let Some(ref s) = *self.shards[port].borrow() {
                        s.add_connect_queue(Arc::new(peer.clone()));
                    }
                }
            }
        }

        // send packets
        self.context.send_packets(actions.send_packets);
        
        Box::new(future::ok(()))
    }

    /// Spawns the threads and puts the networking into a full working state
    pub fn run(config: ClientConfig, rk: Arc<RecordKeeper>, quit: QuitSignal) -> Result<(UnboundedSender<ClientMsg>, thread::JoinHandle<()>), io::Error> {        
        let (tx, rx) = unbounded::<ClientMsg>();

        let t = thread::Builder::new().name("Network Handler".into()).spawn(move || {
            info!("Network Handler thread ready");

            let mut core = Core::new().expect("Could not create network reactor core");
            let (nout, nin) = UdpSocket::bind(&config.bind_addr, &core.handle()).expect("Could not bind P2P socket!").framed(P2PCodec).split();

            let (chain_tx, chain_rx) = unsync::mpsc::unbounded();

            let (rktx, rkrx) = mpsc::channel(10);
            rk.register_record_listener(rktx);

            let t = Rc::new(Client::new(config, rk, &core, chain_tx));

            t.context.sink.set(Some(Box::new(nout)));

            //let this = Rc::clone(&t);

            let mut this = Rc::clone(&t);
            let packet_listener = nin.for_each(move |p| {
                this.process_packet(p);

                future::ok(())
            }).or_else(|e| {

                warn!("Failed to listen to packets: {}", e);

                future::err(())
            });

            this = Rc::clone(&t);
            let msg_handler = rx.for_each(move |p| {
                let f: future::FutureResult<(), ()> = match p {
                    ClientMsg::GetStatistics(r) => future::result(r.send(this.get_stats()).map_err(|_| ())),
                    ClientMsg::GetPeerInfo(r) => future::result(r.send(this.get_peer_info()).map_err(|_| ())),
                    ClientMsg::AddNode(network_id, node) => {
                        let p = this.resolve_port(&network_id);
                        if p < 255  {
                            this.shards[p as usize].borrow().as_ref().unwrap().add_connect_queue(Arc::new(node));                        }

                        future::ok(())
                    },
                    ClientMsg::DropNode(network_id, _) => {
                        let p = this.resolve_port(&network_id);
                        if p < 255 {
                            // not implemented
                        }

                        future::ok(())
                    },
                    ClientMsg::AttachNetwork(network_id, mode) => future::result(this.attach_network(network_id, mode).map(|_| ())),
                    ClientMsg::DetachNetwork(network_id) => {
                        this.detach_network(&network_id);

                        future::ok(())
                    },

                    ClientMsg::ShouldForge(_network_id, r) => {
                        this.clear_weak_jobs();
                        future::result(r.send(this.jobs.borrow().is_empty()).map_err(|_| ()))
                    }
                };

                this.context.event_loop.spawn(f);

                future::ok(())
            });

            this = Rc::clone(&t);
            let chain_handler = chain_rx.for_each(move |(j, augmenter)| {

                // perform job merging if possible, break if we succeeed
                if let Some(h) = augmenter {
                    this.clear_weak_jobs();

                    let mut jobs = this.jobs.borrow_mut();
                    let mut found = false;
                    let mut x = 0;
                    while x < jobs.len() {
                        let oje = jobs[x].upgrade().unwrap(); // guarenteed since we clear weak jobs earlier
                        if oje.get_target() == h {
                            // change the existing job rather than creating a new one.
                            debug!("Augment Existing Job: {}", oje.get_target());
                            oje.augment(j.get_target());
                            found = true;
                            break;
                        }
                        else if Rc::ptr_eq(&j, &oje) {
                            // previous job resubmitted due to failure/continued processing
                            jobs.swap_remove(x);
                            break;
                        }
                        else if j.get_target() == oje.get_target() && !Rc::ptr_eq(&j, &oje) {
                            // duplicate jobs
                            found = true;
                            break;
                        }

                        x += 1;
                    }

                    if found {

                        debug!("Augment/Duplicate Job: Target {}", j.get_target());
                        return Ok::<(), ()>(());
                    }
                }

                if let Some(ref shard) = *this.shards[this.resolve_port(&j.network_id) as usize].borrow() {
                    debug!("Assign Job: {}", j.get_target());
                    if !shard.assign_job(&j) {
                        warn!("Could not assign job in network: {}", j.network_id);
                    }
                    else {
                        this.jobs.borrow_mut().push(Rc::downgrade(&j))
                    }
                }

                Ok::<(), ()>(())
            });

            /*let ntpTask = Interval::new_at(Instant::now(), Duration::from_millis(NODE_NTP_INTERVAL))?
            .and_then(|_| {
                match ntp::calc_drift(this2.config.ntp_servers[0].as_str()) {
                    Ok(drift) => {
                        Time::update_ntp(drift);
                        debug!("NTP time sync completed: drift is {}", drift);
                    },
                    Err(reason) => {
                        warn!("NTP time sync failed: {}", reason);
                    }
                }
            })*/
            
            this = Rc::clone(&t);
            let session_check_task = Interval::new(Duration::from_millis(NODE_CHECK_INTERVAL), &t.context.event_loop)
                .expect("Cannot start network timer!")
                .for_each(move |_| {
                    for i in 0..255 {
                        if let Some(ref s) = *this.shards[i].borrow() {

                            let mut actions = NetworkActions::new(this.as_ref());

                            debug!("Node scan started");
                            s.node_scan(this.context.config.min_nodes as usize, &mut actions);
                            s.check_sessions(&mut actions);

                            this.context.send_packets(actions.send_packets);
                        }
                    }

                    Ok(())
                })
                .or_else(|e| {
                    warn!("Failed to check sessions in timer: {}", e);

                    future::err(())
                });

            this = Rc::clone(&t);
            let rk_task = rkrx.for_each(move |e| {
                match e {
                    RecordEvent::NewBlock {block, fresh: true, ..} => {
                        let shard = this.shards[this.resolve_port(&block.shard) as usize].borrow();

                        if let Some(ref s) = *shard {
                            let mut actions = NetworkActions::new(this.as_ref());
                            s.reliable_flood(Message::NewBlock(block), &mut actions);
                            this.context.send_packets(actions.send_packets);
                        }
                        // otherwise do not propogate anything
                    },
                    RecordEvent::NewTxn {txn, fresh: true} => {
                        // TODO: When we have the ability to tell which network a txn is on, apply to the correct net
                        // for now we assume genesis
                        let shard = this.shards[0].borrow();

                        if let Some(ref s) = *shard {
                            let mut actions = NetworkActions::new(this.as_ref());
                            s.reliable_flood(Message::NewTransaction(txn), &mut actions);
                            this.context.send_packets(actions.send_packets);
                        }
                        // otherwise do not propogate anything
                        
                    },
                    _ => {}
                }

                future::ok(())
            });

            t.context.event_loop.spawn(msg_handler);
            t.context.event_loop.spawn(chain_handler);
            t.context.event_loop.spawn(packet_listener);
            //handle.spawn(ntpTask);
            t.context.event_loop.spawn(session_check_task);
            t.context.event_loop.spawn(rk_task);

            this = Rc::clone(&t);
            core.run(quit).and_then(|_| {
                this.close();

                Ok(())
            }).unwrap(); // technically can never happen

            info!("Network Handler thread completed");
        }).expect("Could not start network handler thread");

        Ok((tx, t))
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

    pub fn get_nodes_from_repo(&self, network_id: &U256, skip: usize, count: usize) -> Vec<Node> {
        let port = self.resolve_port(&network_id);
        if port != 255 {

            let shard = self.shards[port as usize].borrow();

            if let Some(ref s) = *shard {
                return s.get_nodes_from_repo(skip, count);
            }
        }

        Vec::new()
    }

    pub fn get_shard_peer_info(&self, network_id: &U256) -> Vec<SessionInfo> {

        let port = self.resolve_port(network_id);
        if port != 255 {

            let shard = self.shards[port as usize].borrow();

            if let Some(ref s) = *shard {
                return s.get_session_info();
            }
        }

        Vec::new()
    }

    pub fn get_peer_info(&self) -> Vec<SessionInfo> {

        let mut p = Vec::new();

        for i in 0..255 {
            if let Some(ref s) = *self.shards[i].borrow() {
                p.append(&mut s.get_session_info());
            }
        }

        p
    }

    pub fn get_stats(&self) -> Statistics {

        let mut stats = Statistics::new();

        //stats.rx = self.rx.load(Relaxed) as u64;
        //stats.tx = self.tx.load(Relaxed) as u64;

        for i in 0..255 {
            if let Some(ref s) = *self.shards[i].borrow() {
                stats.attached_networks += 1;
                stats.connected_peers += s.session_count() as u32;
            }
        }

        stats
    }

    pub fn get_config(&self) -> &ClientConfig {
        &self.context.config
    }

    pub fn get_record_keeper(&self) -> &Arc<RecordKeeper> {
        &self.context.rk
    }

    pub fn get_handle(&self) -> Handle {
        self.context.event_loop.clone()
    }

    /// Initialize a node repository from file given the ID
    /// NOTE: This is pretty slow, consider using sparingly
    fn load_node_repo(&self, network_id: U256) -> NodeRepository {
        let mut repo = NodeRepository::new();

        let res = repo.load(network_id.to_string().as_str());
        
        if res.is_ok() && res.unwrap() == 0 {
            // add seed nodes
            repo.build(&self.context.config.seed_nodes.iter().cloned().map(|ep| LocalNode::new(Node::new(ep))).collect());
        }

        repo
    }

    fn clear_weak_jobs(&self) {
        let mut x = 0;
        let mut jobs = self.jobs.borrow_mut();
        while x < jobs.len() {
            if jobs[x].upgrade().is_none() {
                jobs.swap_remove(x);
            }
            else {
                x += 1;
            }
        }
    }
}
