use std::collections::{HashMap, VecDeque};
use std::net::UdpSocket;
use std::io::Error;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use std::time::Duration;

use bincode::{serialize, deserialize, Bounded};

use openssl::pkey::PKey;

use super::env::get_client_name;

use u256::*;

use block::*;
use txn::*;
use database::Database;

use network::session::*;
use network::node::*;

use signer::generate_private_key;
use hash::hash_pub_key;

use time::Time;

#[derive(Serialize, Deserialize)]
struct RawPacket {
    payload: Packet
}

//#[derive(Debug)]
pub struct ClientConfig {
    /// Sets a threshold which, at sufficiently low connectivity of nodes (AKA, less than this number), new nodes will be seeked out
    min_nodes: u16,

    /// Sets the maximum simultaneous node connections
    max_nodes: u16,

    /// A private key used to sign and identify our own node data
    private_key: PKey, 
}

impl ClientConfig {
    /// Initializes the config with reasonable defaults
    pub fn new() -> ClientConfig {
        ClientConfig {
            private_key: generate_private_key(),
            min_nodes: 8,
            max_nodes: 16
        }
    }
}

pub struct NetworkContext<'a> {
    /// Repository for nodes, for RO access
    pub node_repo: &'a RwLock<HashMap<U256, NodeRepository>>,

    /// The database for RO access
    pub db: &'a Database,

    /// Queue of transactions to import
    pub import_txns: VecDeque<Txn>,

    /// Queue of blocks to import
    pub import_blocks: VecDeque<Block>
}

impl<'a> NetworkContext<'a> {
    pub fn new(node_repo: &'a RwLock<HashMap<U256, NodeRepository>>, db: &'a Database) -> NetworkContext<'a> {
        NetworkContext {
            node_repo: node_repo,
            db: db,
            import_txns: VecDeque::new(),
            import_blocks: VecDeque::new()
        }
    }
}

pub struct Client<'a> {

    /// Configuration options for the behavior of the network client
    config: ClientConfig,

    /// Independant "connections" to each of the other NodeEndpoints we interact with
    sessions: HashMap<SocketAddr, Session>,

    recv_buf: HashMap<SocketAddr, Vec<u8>>,

    /// The node object which represents my own system
    my_node: Arc<Node>,

    node_repo: RwLock<HashMap<U256, NodeRepository>>,

    db: &'a mut Database,

    /// List of all the networks we should be seeking node connections with
    connected_networks: Vec<U256>,
    /// The socket used to accept and invoke UDP communication
    socket: Option<UdpSocket>,

    last_peer_seek: Time,

    node_idx: usize
}

impl<'a> Client<'a> {

    /// The maximum amount of data that can be in a single message object (the object itself can still be in split into pieces at the datagram level)
    const MAX_PACKET_SIZE: usize = 1024 * 128;

    pub fn new(db: &'a mut Database, config: ClientConfig) -> Client<'a> {
        
        Client {
            db: db,
            my_node: Arc::new(Node {
                key: config.private_key.public_key_to_der().unwrap(), // TODO: Should be public key only!
                version: Session::PROTOCOL_VERSION,
                endpoint: NodeEndpoint { host: String::from(""), port: 0 },
                name: get_client_name()
            }),
            config: config,
            node_repo: RwLock::new(HashMap::new()),
            connected_networks: Vec::new(),
            sessions: HashMap::new(),
            socket: None,
            last_peer_seek: Time::from(0),
            node_idx: 0,
            recv_buf: HashMap::new()
        }

        // Build my node object
    }

    pub fn open(&mut self) -> Result<(), Error> {
        match UdpSocket::bind(self.my_node.endpoint.clone().as_socketaddr().unwrap()) {
            Ok(s) => {

                // socket should read indefinitely
                // this error will crash the program, but it should not fail in normal cases.
                s.set_read_timeout(None).expect("Could not set socket read timeout");

                self.socket = Some(s);

                Ok(())
            },
            Err(e) => Err(e)
        }
    }

    fn node_scan(&mut self, network_id: &U256) {
        let mut removed_nodes = Vec::new();

        if !self.node_repo.read().unwrap().contains_key(network_id) {
            panic!("Tried to scan nodes on nonexistant shard network id: {}", network_id);
        }

        {
            let l = self.node_repo.read().unwrap();
            let node_repo = &l.get(network_id).unwrap();
            if self.sessions.len() < self.config.min_nodes as usize {
                // try to connect a couple more nodes
                'node_search: for _ in 0..3 {

                    let mut peer = node_repo.get_nodes(self.node_idx);
                    let pkh = hash_pub_key(&peer.key[..]);
                    let saddr = peer.endpoint.clone().as_socketaddr();

                    // now we can look into creating a new session
                    if let Some(addr) = saddr {

                        self.node_idx = self.node_idx + 1;

                        let mut n = 0;
                        while self.sessions.contains_key(&addr) {
                            peer = node_repo.get_nodes(self.node_idx);
                            self.node_idx = self.node_idx + 1;
                            n = n + 1;

                            // we have to prevent infinite looping here due to if all the nodes in the DB are conneted to (which is rare)
                            if n > self.sessions.len() {
                                // we must be conneted to all nodes
                                // this should basically only happen if the network itself is so fragmented and too small
                                warn!("Node Repository lacks additional nodes to connect to; node shortage detected.");
                                break 'node_search;
                            }
                        }

                        // ready to connect
                        let sess = Session::new(self.my_node.clone(), peer, addr, network_id.clone());
                        self.sessions.insert(addr, sess);
                    }
                    else {
                        // We have bogus data
                        warn!("Could not resolve hostname for node: {:?}", peer.endpoint);
                        // Remove the bogus data
                        removed_nodes.push(pkh);
                    }
                }
            }
        }

        if !removed_nodes.is_empty() {
            let mut l = self.node_repo.write().unwrap();
            let node_repo = l.get_mut(network_id).unwrap();
            for r in removed_nodes {
                node_repo.remove(&r);
            }
        }
    }

    /// Intended to be run in a single thread. Will receive packets, and react to them if necessary.
    fn recv_loop(&mut self) {
        // Let the sessions work on received packets
        // One kilobyte buffer should handle many packets, but some research is needed
        let mut buf: [u8; 1024] = [0; 1024];
        loop {
            let mut cont = true;

            let mut received_packet: Option<(RawPacket, SocketAddr)> = None;

            match self.socket.as_ref().unwrap().recv_from(&mut buf) {
                Ok((n, addr)) => {

                    let mut remove = false;
                    let mut recv_buf = &mut self.recv_buf;

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
                self.process_packet(&p.payload, &addr);
            }
        };
    }

    /// Evaluate a single packet and route it to a session as necessary
    fn process_packet(&mut self, p: &Packet, addr: &SocketAddr) {
        let mut context = NetworkContext::new(&self.node_repo, &self.db);
        let mut inserted: Option<(SocketAddr, Node, U256)> = None;
        
        match self.sessions.get_mut(&addr) {
            Some(sess) => {
                sess.recv(&p, &mut context);
            },
            None => {
                // session needs to be created
                // special case! we will handle the introduce packet here
                // if it is not an introduce packet, then this ends here.
                // TODO: Make sure that the p.from is not bogus before we start trusting it
                if let Message::Introduce { ref node, ref network_id } = p.msg {
                    inserted = Some((addr.clone(), node.clone(), network_id.clone()));
                }
            }
        }

        if let Some((addr, node, network_id)) = inserted {
            if let Some(repo) = self.node_repo.write().unwrap().get_mut(&network_id) {
                // Here we must check that p.from is who we think it is, otherwise session could be hijacked
                repo.new_node(node.clone());

                // now we can create a new session
                let mut sess = Session::new(self.my_node.clone(), repo.get(&hash_pub_key(&node.key)).unwrap(), addr, network_id);
                sess.recv(&p, &mut context);

                self.sessions.insert(addr, sess);
            }
            // otherwise drop the connection request
        }

        // process data in the context: do we have anything to import?
        // NOTE: Order is important here! We want the data to be imported in order or else it is much harder to construct
        while let Some(txn) = context.import_txns.pop_front() {
            self.report_txn(txn);
        }
        
        while let Some(block) = context.import_blocks.pop_front() {
            self.report_block(block);
        }

        // send any packets pending on the connection
        // session should always be valid at this point.
        let s = &mut self.socket.as_ref().unwrap();
        let sess = self.sessions.get_mut(&addr).unwrap();

        // TODO: Consider rate limiting, which might work well here
        while let Some(p) = sess.pop_send_queue() {

            let mut rawp = RawPacket {
                payload: p
            };

            // TODO: Is it bad that I call this in 2 separate calls, or are they just buffered together?
            let raw = serialize(&rawp, Bounded(Client::MAX_PACKET_SIZE as u64)).unwrap();
            s.send_to(&serialize(&raw.len(), Bounded(4)).unwrap()[..], sess.get_remote_addr());
            s.send_to(&raw[..], sess.get_remote_addr());
        }
    }

    pub fn run(&mut self) {
        // connection management
        /*if self.last_peer_seek.millis() < Time::current().millis() - 3 * 1000 {
            self.node_scan();
            self.last_peer_seek = Time::current();
        }*/

        //let packet_vec = self.read_packets();
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