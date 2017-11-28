use bincode::deserialize;
use openssl::pkey::PKey;
use std::collections::{HashMap, VecDeque};
use std::io::Error;
use std::net::{SocketAddr,UdpSocket};
use std::sync::{Arc, RwLock, Mutex};
use std::sync::atomic::{AtomicBool,AtomicUsize};
use std::sync::atomic::Ordering::Relaxed;
use std::thread;
use std::time::Duration;
use time::Time;

use env::get_client_name;
use network::node::{Node, NodeRepository, NodeEndpoint, LocalNode};
use network::ntp;
use network::session;
use network::session::{RawPacket, Message};
use network::shard::{ShardInfo};
use network::work_controller::NetworkWorkController;
use primitives::{Block, Txn, U256, EventListener};
use record_keeper::{RecordKeeper};
use signer::generate_private_key;
use work_queue::{WorkItem, WorkQueue, WorkResult, Task, WorkResultType, MetaData};


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

const NODE_SCAN_INTERVAL: i64 = 30000; // every 30 seconds
const NODE_CHECK_INTERVAL: i64 = 5000; // every 5 seconds
const NODE_NTP_INTERVAL: i64 = 20 * 60000; // every 20 minutes

/// The maximum amount of data that can be in a single message object (the object itself can still be in split into pieces at the datagram level)
pub const MAX_PACKET_SIZE: usize = 1024 * 128;

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

    pub ntp_servers: Vec<String>,

    pub seed_nodes: Vec<NodeEndpoint>,

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
            port: ClientConfig::DEFAULT_PORT
        }
    }
}

/// Statistical information which can be queried from the network client
#[derive(Debug)]
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

pub struct NetworkContext<'a> {
    /// The database for RO access
    pub rk: &'a RecordKeeper,

    /// The configuation associated with this client
    pub config: &'a ClientConfig,

    pub work_controller: &'a Arc<NetworkWorkController>,

    /// Nodes which can be connected to which were recently supplied
    pub connect_peers: HashMap<U256, Vec<Node>>,
}

impl<'a> NetworkContext<'a> {
    pub fn new(rk: &'a RecordKeeper, config: &'a ClientConfig, work_controller: &'a Arc<NetworkWorkController>) -> NetworkContext<'a> {
        NetworkContext {
            rk,
            config,
            work_controller: work_controller,
            connect_peers: HashMap::new()
        }
    }

    /// Initialize a node repository from file given the ID
    /// NOTE: This is pretty slow, consider using sparingly
    pub fn load_node_repo(&self, network_id: U256) -> NodeRepository {
        let mut repo = NodeRepository::new();

        let res = repo.load(network_id.to_string().as_str());
        
        if res.is_ok() && res.unwrap() == 0 {
            // add seed nodes
            repo.build(&self.config.seed_nodes.iter().cloned().map(|ep| LocalNode::new(Node::new(ep))).collect());
        }

        repo
    }
}

pub struct Client {

    /// Configuration options for the behavior of the network client
    config: ClientConfig,

    /// The node object which represents my own system
    my_node: Arc<Node>,

    /// The record keeper/database
    rk: Arc<RecordKeeper>,

    work_controller: Arc<NetworkWorkController>,

    /// Data structures associated with shard-specific information
    shards: [RwLock<Option<ShardInfo>>; 255],

    /// Number of active shards
    num_shards: u8,

    /// The socket used to accept and invoke UDP communication
    socket: Option<UdpSocket>,

    /// A mutex for sending on the socket--this only applies for sending, since we must always be reading
    socket_mux: Mutex<()>,

    last_peer_seek: Time,

    /// Whether or not we are entered the exit state for the network interface
    done: AtomicBool,

    curr_port: AtomicUsize,

    /// Accumulator reperesnting number bytes received
    rx: AtomicUsize,

    /// Accumulator representing number of bytes sent
    tx: AtomicUsize,
}

impl Client {
    pub fn new(rk: Arc<RecordKeeper>, wq: Arc<WorkQueue>, config: ClientConfig) -> Arc<Client> {
        
        let mut client = Client {
            rk: Arc::clone(&rk),
            work_controller: NetworkWorkController::new(rk, wq),
            shards: init_array!(RwLock<Option<ShardInfo>>, 255, RwLock::new(None)),
            num_shards: 0,
            my_node: Arc::new(Node {
                key: config.private_key.public_key_to_der().unwrap(), // TODO: Should be public key only!
                version: session::PROTOCOL_VERSION,
                endpoint: NodeEndpoint { host: config.hostname.clone(), port: config.port },
                name: get_client_name()
            }),
            config: config,
            socket: None,
            socket_mux: Mutex::new(()),
            last_peer_seek: Time::from(0),
            done: AtomicBool::new(false),
            curr_port: AtomicUsize::new(0),
            rx: AtomicUsize::new(0),
            tx: AtomicUsize::new(0),
        };

        client.open().expect("Could not open client!");
        let c = Arc::new(client);
        
        c
    }

    /// Connect to the specified shard by shard ID. On success, returns the number of pending connections (the number of nodes)
    /// A result value of 0 does not indicate failure; it simply means that we need some time to gain connections within the net.
    /// Be patient.
    pub fn attach_network(&self, network_id: U256, mode: ShardMode) -> Result<usize, ()> {

        if self.num_shards > 128 {
            // we risk overwhelming the ports
            return Err(());
        }

        // first, setup the node repository
        let repo = NetworkContext::new(&self.rk, &self.config, &self.work_controller).load_node_repo(network_id);
        let node_count = repo.len();

        debug!("Attached network repo size: {}", node_count);

        // find a suitable port
        let mut port;
        loop {
            port = (self.curr_port.fetch_add(1, Relaxed) % 255) as u8;

            // make sure the port is not taken (this should almost always take one try)
            let sh = self.shards[port as usize].read().unwrap();
            match *sh {

                None => break,
                _ => {}
            }
        }

        // we can now get going
        let si = ShardInfo::new(network_id, port, mode, repo);

        let mut shard = self.shards[port as usize].write().unwrap();
        *shard = Some(si);

        // TODO: Constant?
        if node_count >= 2 {
            // we can start connecting to nodes immediately
            Ok(shard.as_mut().unwrap().node_scan(&self.my_node, 8))
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

        let mut sh = self.shards[idx as usize].write().unwrap();
        if let None = *sh {
            return false;
        }

        sh.as_ref().unwrap().close(&self.socket.as_ref().unwrap());
        *sh = None;

        true
    }

    fn resolve_port(&self, network_id: &U256) -> u8 {
        for i in 0..255 {
            let shard = self.shards[i].read().unwrap();
            if let Some(ref sh) = *shard {
                if sh.get_network_id() == network_id {
                    return i as u8;
                }
            }
        }

        255
    }

    pub fn open(&mut self) -> Result<(), Error> {
        match UdpSocket::bind(self.my_node.endpoint.clone().as_socketaddr().expect("Could not parse hostname... is it valid?")) {
            Ok(s) => {

                // socket should read indefinitely
                // but we need to break every now and then to check thread signals
                s.set_read_timeout(Some(Duration::from_millis(100))).expect("Could not set socket read timeout");

                self.socket = Some(s);

                Ok(())
            },
            Err(e) => Err(e)
        }
    }

    /// Intended to be run in a single thread. Will receive packets, and react to them if necessary.
    fn recv_loop(&self) {
        // Let the sessions work on received packets
        // One kilobyte buffer should handle many packets, but some research is needed
        let mut buf: [u8; 1024] = [0; 1024];
        let mut recv_buf: HashMap<SocketAddr, Vec<u8>> = HashMap::new();

        loop {
            let mut cont = true;

            let mut received_packet: Option<(RawPacket, SocketAddr)> = None;

            match self.socket.as_ref().unwrap().recv_from(&mut buf) {
                Ok((n, addr)) => {

                    let mut remove = false;

                    let recv_len = recv_buf.len();

                    let mut newb: Option<(SocketAddr, Vec<u8>)> = None;
                    if let Some(b) = recv_buf.get_mut(&addr) {
                        if b.len() > MAX_PACKET_SIZE {
                            // cannot accept more packet data from this client
                            warn!("Client exceeded max packet size, dumping: {:?}", addr);
                            remove = true;
                            cont = false;
                        }
                        // use memory
                        b.extend_from_slice(&buf[..n]);
                    }
                    else {
                        // new peer, can we take more?
                        if recv_len <= self.config.max_nodes as usize {
                            // allocate
                            let mut v: Vec<u8> = Vec::with_capacity(MAX_PACKET_SIZE);
                            v.extend_from_slice(&buf[..n]);
                            newb = Some((addr, v));
                        }
                        else {
                            continue;
                        }
                    }

                    if remove { //TODO: Check that remove and cont checks are in the correct places, possibly remove booleans and just insert the code in place.
                        recv_buf.remove(&addr);
                    }
                    if !cont {
                        continue;
                    }

                    self.rx.fetch_add(n, Relaxed);

                    if let Some((addr, v)) = newb {
                        recv_buf.insert(addr, v);
                    }

                    // have we received all the data yet?
                    let b = recv_buf.get_mut(&addr).unwrap();
                    let len: usize = deserialize::<u32>(&b[0..4]).unwrap() as usize;

                    if b.len() - 4 >= len {
                        // we have a full packet, move it into our packet list
                        match deserialize(&b[4..]) {
                            Ok(p) => received_packet = Some((p, addr)),
                            Err(e) => {
                                warn!("Packet decode failed from {}: {}", addr, e);
                                warn!("Expected size: {}", len);
                                // TODO: Note misbehaviors/errors
                            }
                        };

                        b.clear();
                    }
                },
                Err(err) => {}
            }

            if let Some((p, addr)) = received_packet {
                //debug!("Got packet: {:?}, {:?}", p, addr);
                if p.port == 255 {
                    if let Message::Introduce { ref node, ref network_id, .. } = p.payload.msg {
                        // new session?
                        let idx = self.resolve_port(network_id);
                        if let Some(ref shard) = *self.shards[idx as usize].read().unwrap() {
                            if let Ok(addr) = shard.open_session(Arc::new(node.clone()), self.my_node.clone(), Some(&p.payload)) {
                                info!("New contact opened from {}", addr);
                                let lock = self.socket_mux.lock();
                                self.tx.fetch_add(shard.send_packets(&addr, &self.socket.as_ref().unwrap()) as usize, Relaxed);
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
                else if let Some(ref shard) = *self.shards[p.port as usize].read().unwrap() {
                    let mut context = NetworkContext::new(&self.rk, &self.config, &self.work_controller);

                    {
                        shard.process_packet(&p.payload, &addr, &mut context);
                        let lock = self.socket_mux.lock();
                        self.tx.fetch_add(shard.send_packets(&addr, &self.socket.as_ref().unwrap()) as usize, Relaxed);
                    }

                    // finally, any new nodes to connect to?
                    for (network_id, peers) in context.connect_peers.iter() {
                        for peer in peers {
                            // for right now, only save for nodes of open networks
                            let port = self.resolve_port(network_id) as usize;
                            if port != 255 {
                                // add to the connect queue of the network
                                // a successful connection will result in the node being added to the permanent db
                                let shard = self.shards[port].read().unwrap();
                                if let Some(ref sh) = *shard {

                                    sh.add_connect_queue(Arc::new(peer.clone()));
                                }
                            }
                        }
                    }
                }
                else {
                    // bogus network ID received, ignore
                    // TODO: A good debug print here might also print the packet
                    debug!("Received unregistered network port packet: {}", p.port);
                }
            }

            if self.done.load(Relaxed) {
                // stop receiving packets
                break;
            }
        };
    }

    /// Spawns the threads and puts the networking into a full working state
    pub fn run(this: Arc<Client>) -> Vec<thread::JoinHandle<()>> {

        if this.done.load(Relaxed) {
            panic!("Tried to run network after already closed");
        }

        let mut joins: Vec<thread::JoinHandle<()>> = Vec::new();

        let this2 = this.clone();
        // TODO: Do something about thread references! We need to be able to join to shut down the network thread
        joins.push(thread::Builder::new().name("P2P Handler".into()).spawn(move || {
            info!("P2P Handler thread ready");
            this.recv_loop();
            info!("P2P Handler thread completed");
        }).expect("Could not start P2P handler thread"));
        joins.push(thread::Builder::new().name("Net Discovery/Maintenance".into()).spawn(move || {

            info!("Node discovery/maintenance thread ready");

            let mut last_ntp_scan = Time::from_seconds(0);
            let mut last_node_scan = Time::from_seconds(0);
            let mut last_node_check = Time::from_seconds(0);

            loop {

                if this2.done.load(Relaxed) {
                    break;
                }

                let n = Time::current_local();

                if last_ntp_scan.diff(&n).millis() > NODE_NTP_INTERVAL && !this2.config.ntp_servers.is_empty() {
                    // TODO: Choose a random NTP server rather than only the first
                    match ntp::calc_drift(this2.config.ntp_servers[0].as_str()) {
                        Ok(drift) => {
                            Time::update_ntp(drift);
                            debug!("NTP time sync completed: drift is {}", drift);
                        },
                        Err(reason) => {
                            warn!("NTP time sync failed: {}", reason);
                        }
                    }

                    last_ntp_scan = n;
                }

                if last_node_check.diff(&n).millis() > NODE_CHECK_INTERVAL {
                    for i in 0..255 {
                        if let Some(ref mut s) = *this2.shards[i].write().unwrap() {
                            if last_node_scan.diff(&n).millis() > NODE_SCAN_INTERVAL {
                                s.node_scan(&this2.my_node, this2.config.min_nodes as usize);
                            }

                            s.check_sessions();

                            // send packets that may have accumulated
                            let lock = this2.socket_mux.lock();
                            this2.tx.fetch_add(s.send_all_packets(&this2.socket.as_ref().unwrap()), Relaxed);
                        }
                    }

                    last_node_check = n;
                }

                if last_node_scan.diff(&n).millis() > NODE_SCAN_INTERVAL {
                    // tell all networks to connect to more nodes
                    debug!("Node scan started");

                    last_node_scan = n;
                }

                thread::sleep(::std::time::Duration::from_millis(1000));
            }

            info!("Node discovery thread completed");
        }).expect("Could not start node discovery thread"));

        joins
    }

    /// End all network resources and prepare for program close
    /// You are still responsible for joining to the network threads to make sure they close properly
    pub fn close(&self) {

        debug!("Closing network...");

        self.done.store(true, Relaxed);

        // detach all networks
        for i in 0..255 {
            let exists = self.shards[i].read().unwrap().is_some();

            if exists {
                self.detach_network_port(i as u8);
            }
        }
    }

    pub fn get_stats(&self) -> Statistics {

        let mut stats = Statistics::new();

        stats.rx = self.rx.load(Relaxed) as u64;
        stats.tx = self.tx.load(Relaxed) as u64;

        for i in 0..255 {
            if let Some(ref s) = *self.shards[i].read().unwrap() {
                stats.attached_networks += 1;
                stats.connected_peers += s.session_count() as u32;
            }
        }

        stats
    }

    pub fn get_config(&self) -> &ClientConfig {
        &self.config
    }
}