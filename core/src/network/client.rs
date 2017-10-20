use bincode::{serialize, deserialize, Bounded};
use hash::hash_pub_key;
use network::node::*;
use network::session::*;
use openssl::pkey::PKey;
use record_keeper::block::*;
use record_keeper::database::Database;
use record_keeper::txn::*;
use signer::generate_private_key;
use std::cmp::min;
use std::collections::{HashMap, VecDeque};
use std::io::Error;
use std::net::{SocketAddr,UdpSocket};
use std::sync::{Arc, RwLock, Mutex};
use std::sync::atomic::{AtomicBool,AtomicUsize};
use std::sync::atomic::Ordering::Relaxed;
use std::thread;

use std::time::Duration;
use super::env::get_client_name;

use u160::*;
use u256::*;

use time::Time;

use network::session::*;
use network::node::*;
use network::ntp::*;

const NODE_SCAN_INTERVAL: i64 = 30000; // every 30 seconds
const NODE_NTP_INTERVAL: i64 = 20 * 60000; // every 20 minutes

#[derive(Serialize, Deserialize)]
struct RawPacket {
    /// Which communication channel should be regarded for this node.
    /// This is included so nodes can have multiple connections to each other through separate shards
    /// Port 255 is reserved for connecting from remote nodes when the local port is unknown
    pub port: u8,
    /// The data which should be delivered to the session handler
    pub payload: Packet
}

//#[derive(Debug)]
pub struct ClientConfig {
    /// Sets a threshold which, at sufficiently low connectivity of nodes (AKA, less than this number), new nodes will be seeked out
    pub min_nodes: u16,

    /// Sets the maximum simultaneous node connections
    pub max_nodes: u16,

    pub ntp_servers: Vec<String>,

    /// A private key used to sign and identify our own node data
    pub private_key: PKey, 
}

impl ClientConfig {
    /// Initializes the config with reasonable defaults
    pub fn new() -> ClientConfig {
        ClientConfig {
            private_key: generate_private_key(),
            ntp_servers: vec!["pool.ntp.org".into()],
            min_nodes: 8,
            max_nodes: 16
        }
    }
}

pub struct NetworkContext<'a> {
    /// The database for RO access
    pub db: &'a Database,

    /// Queue of transactions to import
    pub import_txns: VecDeque<Txn>,

    /// Queue of blocks to import
    pub import_blocks: VecDeque<Block>,

    /// Nodes which can be connected to which were recently supplied
    pub connect_peers: HashMap<U256, Vec<Node>>,
}

impl<'a> NetworkContext<'a> {
    pub fn new(db: &'a Database) -> NetworkContext<'a> {
        NetworkContext {
            db: db,
            import_txns: VecDeque::new(),
            import_blocks: VecDeque::new(),
            connect_peers: HashMap::new()
        }
    }

    /// Initialize a node repository from file given the ID
    /// NOTE: This is pretty slow, consider using sparingly
    pub fn load_node_repo(network_id: U256) -> NodeRepository {
        let mut repo = NodeRepository::new();
        repo.load(network_id.to_string().as_str()).unwrap_or(0);

        repo
    }
}

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

struct ShardInfo {
    /// Unique identifier for this shard (usually genesis block hash)
    network_id: U256,

    /// The port which should be assigned to clients
    pub port: u8,

    /// Functional requirements of this shard connection
    pub mode: ShardMode,

    /// Independant "connections" to each of the other NodeEndpoints we interact with
    pub sessions: RwLock<HashMap<SocketAddr, Session>>,

    /// The index of the node we should scan next in the node repository. Incremented for each connection attempt
    last_peer_idx: usize,

    /// Collection of nodes which can be connected to this on this network
    node_repo: NodeRepository,
}

impl ShardInfo {
    pub fn new(network_id: U256, port: u8, mode: ShardMode, repo: NodeRepository) -> ShardInfo {
        ShardInfo {
            network_id: network_id,
            port: port,
            mode: mode,
            sessions: RwLock::new(HashMap::new()),
            last_peer_idx: 0,
            node_repo: repo
        }
    }

    /// Try to scan the node database and ensure the minimum requested number of nodes
    pub fn node_scan(&mut self, my_node: &Arc<Node>, wanted: usize) -> usize {
        let mut removed_nodes = Vec::new();

        {
            let mut attempts = 0;
            while self.sessions.read().unwrap().len() < wanted {
                info!("Reaching for new nodes...");
                // try to connect a couple more nodes

                let mut peer = self.node_repo.get_nodes(self.last_peer_idx);
                self.last_peer_idx += 1;
                
                let sess: Option<(SocketAddr, Session)> = match self.open_session(my_node.clone(), peer.clone(), None) {
                    Ok(sopt) => Some(sopt),
                    Err(_) => {
                        removed_nodes.push(hash_pub_key(&peer.key[..]));
                        None
                    }
                };

                if let Some((addr, sess)) = sess {
                    self.sessions.write().unwrap().insert(addr, sess);
                }

                if attempts >= wanted * 3 || attempts > self.node_repo.len() {
                    break;
                }
                attempts += 1;
            }
        }

        if !removed_nodes.is_empty() {
            for r in removed_nodes {
                self.node_repo.remove(&r);
            }
        }

        self.sessions.read().unwrap().len()
    }

    pub fn open_session(&self, peer: Arc<Node>, my_node: Arc<Node>, introduce: Option<&Packet>) -> Result<(SocketAddr, Session), ()> {
        let pkh = hash_pub_key(&peer.key[..]);
        let saddr = peer.endpoint.clone().as_socketaddr();

        // now we can look into creating a new session
        if let Some(addr) = saddr {
            if self.sessions.read().unwrap().contains_key(&addr) {
                return Err(()); // already connected
            }

            // ready to connect
            let sess = Session::new(my_node.clone(), self.port, peer, addr, self.network_id.clone(), introduce);
            Ok((addr, sess))
        }
        else {
            // We have bogus data
            warn!("Could not resolve hostname for node: {:?}", peer.endpoint);
            Err(())
        }
    }

    /// Call to set this shard to a state where all nodes are disconnected and data should stop being validated/tracked
    pub fn close(&self, s: &UdpSocket) {
        for (addr, sess) in self.sessions.read().unwrap().iter() {
            sess.close();
            self.send_packets(addr, s);
        }
    }

    /// Evaluate a single packet and route it to a session as necessary
    pub fn process_packet(&self, p: &Packet, addr: &SocketAddr, mut context: &mut NetworkContext, socket: &UdpSocket) {
        {
            match self.sessions.write().unwrap().get_mut(&addr) {
                Some(sess) => {
                    sess.recv(&p, &mut context);
                },
                None => {
                    // should never happen because all sessions init through port 255
                }
            }
        }

        // send any packets pending on the connection
        // session should always be valid at this point.

        // TODO: Consider rate limiting, which might work well here
        self.send_packets(&addr, socket);
    }

    /// Send all the packets queued for the given session
    pub fn send_packets(&self, addr: &SocketAddr, s: &UdpSocket) -> usize {
        let mut count: usize = 0;

        let mut sr = self.sessions.write().unwrap();
        let sess = sr.get_mut(addr).unwrap();

        while let Some(p) = sess.pop_send_queue() {

            let mut rawp = RawPacket {
                port: sess.get_remote().1,
                payload: p
            };

            // TODO: Is it bad that I call this in 2 separate calls, or are they just buffered together?
            let raw = serialize(&rawp, Bounded(Client::MAX_PACKET_SIZE as u64)).unwrap();
            s.send_to(&serialize(&raw.len(), Bounded(4)).unwrap()[..], sess.get_remote_addr());
            s.send_to(&raw[..], sess.get_remote_addr());

            count += 1;
        }

        count
    }

    pub fn send_all_packets(&self, s: &UdpSocket) -> usize {
        let mut sent = 0;
        for addr in self.sessions.read().unwrap().keys() {
            sent += self.send_packets(addr, s);
        }

        sent
    }

    pub fn get_network_id(&self) -> &U256 {
        return &self.network_id;
    }

    pub fn get_node_repo(&self) -> &NodeRepository {
        return &self.node_repo;
    }
}

pub struct Client {

    /// Configuration options for the behavior of the network client
    config: ClientConfig,

    /// The node object which represents my own system
    my_node: Arc<Node>,

    /// The database
    db: Arc<Database>,

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

    curr_port: AtomicUsize
}

impl Client {

    /// The maximum amount of data that can be in a single message object (the object itself can still be in split into pieces at the datagram level)
    const MAX_PACKET_SIZE: usize = 1024 * 128;

    pub fn new(db: Arc<Database>, config: ClientConfig) -> Client {
        
        Client {
            db: db,
            shards: init_array!(RwLock<Option<ShardInfo>>, 255, RwLock::new(None)),
            num_shards: 0,
            my_node: Arc::new(Node {
                key: config.private_key.public_key_to_der().unwrap(), // TODO: Should be public key only!
                version: Session::PROTOCOL_VERSION,
                endpoint: NodeEndpoint { host: String::from(""), port: 0 },
                name: get_client_name()
            }),
            config: config,
            socket: None,
            socket_mux: Mutex::new(()),
            last_peer_seek: Time::from(0),
            done: AtomicBool::new(false),
            curr_port: AtomicUsize::new(0)
        }
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
        let repo = NetworkContext::load_node_repo(network_id);
        let node_count = repo.len();

        // find a suitable port
        let mut port = 0;
        loop {
            port = (self.curr_port.fetch_add(1, Relaxed) % 255) as u8;

            // make sure the port is not taken (this should almost always take one try)
            let mut sh = self.shards[port as usize].read().unwrap();
            match *sh {

                None => break,
                _ => {}
            }
        }

        // we can now get going
        let mut si = ShardInfo::new(network_id, port, mode, repo);

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
        let idx = self.resolve_port(network_id);

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
        match UdpSocket::bind(self.my_node.endpoint.clone().as_socketaddr().unwrap()) {
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
                        if b.len() > Client::MAX_PACKET_SIZE {
                            // cannot accept more packet data from this client
                            warn!("Client exceeded max packet size, dumping: {:?}", addr);
                            remove = true;
                            continue;
                        }
                        // use memory
                        b.extend_from_slice(&buf[..n]);
                    }
                    else {
                        // new peer, can we take more?
                        if recv_len <= self.config.max_nodes as usize {
                            // allocate
                            let mut v: Vec<u8> = Vec::with_capacity(Client::MAX_PACKET_SIZE);
                            v.extend_from_slice(&buf[..n]);
                            newb = Some((addr, v));
                        }
                        else {
                            continue;
                        }
                    }

                    if let Some((addr, v)) = newb {
                        recv_buf.insert(addr, v);
                    }

                    if remove {
                        recv_buf.remove(&addr);
                    }

                    // have we received all the data yet?
                    let b = recv_buf.get_mut(&addr).unwrap();
                    let len: usize = deserialize::<u32>(&b[0..4]).unwrap() as usize;

                    if b.len() - 4 >= len {
                        // we have a full packet, move it into our packet list
                        match deserialize(&b[..]) {
                            Ok(p) => received_packet = Some((p, addr)),
                            Err(e) => {
                                warn!("Packet decode failed from {}: {}", addr, e);
                                // TODO: Note misbehaviors/errors
                            }
                        };

                        b.clear();
                    }
                },
                Err(err) => break
            }

            if let Some((p, addr)) = received_packet {
                if let Some(ref shard) = *self.shards[p.port as usize].read().unwrap() {
                    let mut context = NetworkContext::new(&self.db);

                    {
                        let lock = self.socket_mux.lock();
                        shard.process_packet(&p.payload, &addr, &mut context, &self.socket.as_ref().unwrap());
                    }

                    // process data in the context: do we have anything to import?
                    // NOTE: Order is important here! We want the data to be imported in order or else it is much harder to construct
                    while let Some(txn) = context.import_txns.pop_front() {
                        self.report_txn(txn);
                    }
                    
                    while let Some(block) = context.import_blocks.pop_front() {
                        self.report_block(block);
                    }

                    // finally, any new nodes to connect to?
                    // TODO: Put in
                }
                else if p.port == 255 {
                    if let Message::Introduce { ref node, ref network_id, ref port } = p.payload.msg {
                        // new session?
                        let idx = self.resolve_port(network_id);
                        if let Some(ref shard) = *self.shards[idx as usize].read().unwrap() {
                            shard.open_session(Arc::new(node.clone()), self.my_node.clone(), Some(&p.payload));
                        }
                        else {
                            debug!("Invalid network ID received in join for network: {}", network_id);
                        }
                    }
                    else {
                        debug!("Received non-introduce first packet on generic port!");
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
    pub fn run(this: Arc<Client>) {
        let this2 = this.clone();
        // TODO: Do something about thread references! We need to be able to join to shut down the network thread
        thread::Builder::new().name("P2P Handler".into()).spawn(move || this.recv_loop());
        thread::Builder::new().name("Net Discovery/Maintenance".into()).spawn(move || {
            let mut last_ntp_scan = Time::from_seconds(0);
            let mut last_node_scan = Time::from_seconds(0);

            loop {
                let n = Time::current_local();

                if last_ntp_scan.diff(&n).millis() > NODE_NTP_INTERVAL && !this2.config.ntp_servers.is_empty() {
                    // TODO: Choose a random NTP server rather than only the first
                    match calc_ntp_drift(this2.config.ntp_servers[0].as_str()) {
                        Ok(drift) => {
                            Time::update_ntp(drift);
                            debug!("NTP time sync completed: {}", drift);
                        },
                        Err(()) => {
                            warn!("NTP time sync failed");
                        }
                    }

                    last_ntp_scan = n;
                }

                if last_node_scan.diff(&n).millis() > NODE_SCAN_INTERVAL {
                    // tell all networks to connect to more nodes
                    last_node_scan = n;
                }

                thread::sleep(::std::time::Duration::from_millis(1000));
            }
        });
    }

    pub fn report_txn(&self, txn: Txn) {
        // do we already have this txn? if so, stop here


        // validate txn


        // save txn to pool (whether that is the db or whatever)


        // reliable flood, since this is a new txn
    }

    pub fn report_block(&self, block: Block) {
        // do we already have this block? if so, stop here


        // validate block


        // save block to pool (whether that is the db or whatever)


        // reliable flood, since this is a new block
    }
}