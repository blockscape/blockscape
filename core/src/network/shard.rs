use std::cmp::min;
use bincode::{serialize, Bounded};
use rand;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::{SocketAddr,UdpSocket};
use std::io;
use std::sync::{Mutex,RwLock};
use std::sync::Arc;
use std::ops::Deref;

use network::client::{ShardMode, NetworkContext};
use network::client;
use network::node::{Node, NodeRepository};
use network::session::{Session, Message, SessionInfo, ByeReason, Packet, RawPacket};
use primitives::{U256, U160};

pub struct ShardInfo {
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

    connect_queue: Mutex<HashSet<Arc<Node>>>
}

impl ShardInfo {
    pub fn new(network_id: U256, port: u8, mode: ShardMode, repo: NodeRepository) -> ShardInfo {
        ShardInfo {
            network_id: network_id,
            port: port,
            mode: mode,
            sessions: RwLock::new(HashMap::new()),
            last_peer_idx: 0,
            node_repo: repo,
            connect_queue: Mutex::new(HashSet::new())
        }
    }

    /// Try to scan the node database and ensure the minimum requested number of nodes
    pub fn node_scan(&mut self, my_node: &Arc<Node>, wanted: usize) -> usize {

        let cur_count = self.sessions.read().unwrap().len();

        if cur_count >= wanted {
            return cur_count; // no need to do any more
        }

        // Ask a random peer for more nodes, to keep the database saturated
        {
            let s = self.sessions.read().unwrap();
            if !s.is_empty() {
                let mut rng = rand::thread_rng();
                let sess = rand::sample(&mut rng, s.values(), 1)[0];

                sess.find_nodes(&self.network_id);
            }
        }

        let mut queue = self.connect_queue.lock().unwrap();

        // do we need more nodes to connect to (from the queue)? If so, pull from the node repo
        {

            info!("Starting new node queue size: {}, repo size: {}", queue.len(), self.node_repo.len());

            let mut attempts = 0;
            if self.node_repo.len() > 0 {
                while cur_count + queue.len() < wanted {
                    let peer = self.node_repo.get_nodes(self.last_peer_idx);
                    self.last_peer_idx += 1;

                    queue.insert(peer);

                    attempts += 1;

                    if attempts >= wanted * 3 || attempts >= self.node_repo.len() {
                        break;
                    }
                }
            }

            info!("Reaching for {} new nodes...", queue.len());
        }

        let mut removed_nodes: Vec<U160> = Vec::new();

        // pull from the connection queue

        for peer in queue.drain() {
            match self.open_session(peer.clone(), my_node.clone(), None) {
                Ok(sopt) => {
                    info!("New contact opened to {}", sopt);
                    Some(sopt)
                },
                Err(_) => {
                    removed_nodes.push(peer.get_hash_id());
                    None
                }
            };
        }

        for r in removed_nodes {
            debug!("Remove broken node from repo: {:?}", r);
            self.node_repo.remove(&r);
        }

        let r = self.node_repo.save(format!("{}", self.network_id).as_str());

        if r.is_err() {
            warn!("Failed to save nodes to file: {:?}", r.unwrap_err());
        }
        else {
            debug!("Saved {} nodes from repo", r.unwrap());
        }

        self.sessions.read().unwrap().len()
    }

    /// Sends pings and removes dead connections as necessary
    pub fn check_sessions(&mut self) {

        let mut removed: Vec<SocketAddr> = Vec::new();


        let mut s = self.sessions.write().unwrap();

        for (addr, sess) in s.iter_mut() {
            sess.check_conn();

            if let Some(d) = sess.is_done() {
                removed.push(addr.clone());

                debug!("Remove session: {:?}", sess.get_remote_node().endpoint);

                // may have to do something additional depending on the failure reason:
                match d {
                    ByeReason::ExitPermanent => {
                        // remove this node from the db as well
                        self.node_repo.remove(
                            &sess.get_remote_node().get_hash_id()
                        );
                    },
                    // TODO: Maybe also add the node to a blacklist of some kind?
                    ByeReason::Abuse => {
                        // remove this node from the db as well
                        self.node_repo.remove(
                            &sess.get_remote_node().get_hash_id()
                        );
                    },
                    _ => {}
                }
            }

            // TODO: for now this is a little inefficient (requires a U160 hash for each client every 5 seconds), but it works
            // add introduced nodes to the repo
            if sess.is_introduced() && self.node_repo.get(&sess.get_remote_node().get_hash_id()).is_none() {
                debug!("Add node to DB: {:?}", addr);
                self.node_repo.new_node(
                    sess.get_remote_node().deref().clone()
                );
            }
        }

        for remove in removed {
            s.remove(&remove);
        }
    }

    pub fn open_session(&self, peer: Arc<Node>, my_node: Arc<Node>, introduce: Option<&Packet>) -> Result<SocketAddr, ()> {
        
        // filter out obvious failure cases:
        // connect to self?
        if peer.endpoint == my_node.endpoint {
            return Err(());
        }
        
        let saddr = peer.endpoint.clone().as_socketaddr();

        // now we can look into creating a new session
        if let Some(addr) = saddr {

            if self.sessions.read().unwrap().contains_key(&addr) {
                return Err(()); // already connected
            }

            debug!("New session: {:?}", peer.endpoint);

            // readyf to connect
            let sess = Session::new(my_node.clone(), self.port, peer, addr, self.network_id.clone(), introduce);

            self.sessions.write().unwrap().insert(addr, sess);

            Ok(addr)
        }
        else {
            // We have bogus data
            warn!("Could not resolve hostname for node: {:?}", peer.endpoint);
            Err(())
        }
    }

    pub fn add_connect_queue(&self, node: Arc<Node>) {
        let mut queue = self.connect_queue.lock().unwrap();

        // avoid duplicates
        queue.insert(node);
    }

    /// Call to set this shard to a state where all nodes are disconnected and data should stop being validated/tracked
    pub fn close(&self, s: &UdpSocket) {
        debug!("Close shard: {}", self.network_id);
        for sess in self.sessions.write().unwrap().values_mut() {
            sess.close();
            if let Err(e) = Self::send_session_packets(sess, s) {
                warn!("Failed to send connection close packet: {:?}", e);
            }
        }
    }

    /// Evaluate a single packet and route it to a session as necessary
    pub fn process_packet(&self, p: &Packet, addr: &SocketAddr, mut context: &mut NetworkContext) {
        
        match p.msg {
            Message::Ping { .. } => {},
            Message::Pong { .. } => {},
            _ => debug!("{} ==> {:?}", addr, &p)
        };
        
        match self.sessions.write().unwrap().get_mut(&addr) {
            Some(sess) => {
                sess.recv(&p, &mut context);
            },
            None => {
                // should never happen because all sessions init through port 255
                warn!("Unroutable packet: {:?}", p);
            }
        }
    }

    /// Send all the packets queued for the given session. Returns number of bytes sent.
    pub fn send_packets(&self, addr: &SocketAddr, s: &UdpSocket) -> Result<usize, io::Error> {
        let mut sr = self.sessions.write().unwrap();
        
        if let Some(mut sess) = sr.get_mut(addr) {
            Self::send_session_packets(&mut sess, s)
        }
        else {
            Ok(0)
        }
    }

    /// Send all the packets queued for any session in this network. Returns number of bytes sent.
    pub fn send_all_packets(&self, s: &UdpSocket) -> Result<usize, io::Error> {
        let mut sent: usize = 0;
        for mut sess in self.sessions.write().unwrap().values_mut() {
            sent += Self::send_session_packets(&mut sess, s)? as usize;
        }

        Ok(sent)
    }

    pub fn get_network_id(&self) -> &U256 {
        return &self.network_id;
    }

    pub fn session_count(&self) -> usize {
        // filter only sessions which are past introductions
        let mut count = 0;
        for sess in self.sessions.read().unwrap().values() {
            if sess.is_introduced() {
                count += 1;
            }
        }

        count
    }

    /// Returns information on all active sessions
    pub fn get_session_info(&self) -> Vec<SessionInfo> {

        let mut vec = Vec::new();

        for sess in self.sessions.read().unwrap().values() {
            if sess.is_introduced() {
                vec.push(sess.get_info());
            }
        }

        vec
    }

    fn send_session_packets(sess: &mut Session, s: &UdpSocket) -> Result<usize, io::Error> {
        let mut count: usize = 0;

        while let Some(p) = sess.pop_send_queue() {

            match p.msg {
                Message::Ping { .. } => {},
                Message::Pong { .. } => {},
                _ => debug!("{} <== {:?}", sess.get_remote_addr(), &p)
            };

            let rawp = RawPacket {
                port: sess.get_remote().1,
                payload: p
            };

            // TODO: Is it bad that I call this in 2 separate calls, or are they just buffered together?
            let raw = serialize(&rawp, Bounded(client::MAX_PACKET_SIZE as u64)).unwrap();
            let size_enc = serialize(&(raw.len() as u32), Bounded(4)).unwrap();
            s.send_to(&size_enc[..], sess.get_remote_addr())?;
            s.send_to(&raw[..], sess.get_remote_addr())?;

            count += 4 + raw.len() as usize;
        }

        Ok(count)
    }

    pub fn get_nodes_from_repo(&self, skip: usize, count: usize) -> Vec<Node> {

        let mut nodes: Vec<Node> = Vec::with_capacity(min(count, self.node_repo.len()));

        for i in skip..min(self.node_repo.len(), skip + count) {
            // dont send the node if it is self
            let d = self.node_repo.get_nodes((skip + i) as usize);
            let n = d.deref();

            nodes.push(n.clone());
        }

        nodes
    }
}