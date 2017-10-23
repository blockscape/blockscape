use std::collections::HashMap;
use std::net::{SocketAddr,UdpSocket};
use std::sync::{Mutex,RwLock};
use std::sync::Arc;
use std::collections::VecDeque;

use bincode::{serialize, Bounded};

use primitives::u256::*;
use primitives::u160::*;

use network::client::*;
use network::session::*;
use network::node::*;

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

    connect_queue: Mutex<VecDeque<Arc<Node>>>
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
            connect_queue: Mutex::new(VecDeque::new())
        }
    }

    /// Try to scan the node database and ensure the minimum requested number of nodes
    pub fn node_scan(&mut self, my_node: &Arc<Node>, wanted: usize) -> usize {

        let cur_count = self.sessions.read().unwrap().len();

        if cur_count >= wanted {
            return cur_count; // no need to do any more
        }

        // do we need more nodes to connect to (from the queue)? If so, pull from the node repo
        {
            let mut queue = self.connect_queue.lock().unwrap();

            info!("Starting new node queue size: {}, repo size: {}", queue.len(), self.node_repo.len());

            let mut attempts = 0;
            if self.node_repo.len() > 0 {
                while cur_count + queue.len() < wanted {
                    let peer = self.node_repo.get_nodes(self.last_peer_idx);
                    self.last_peer_idx += 1;

                    queue.push_back(peer);

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

        while let Some(peer) = self.connect_queue.lock().unwrap().pop_front() {
            let sess: Option<SocketAddr> = match self.open_session(peer.clone(), my_node.clone(), None) {
                Ok(sopt) => Some(sopt),
                Err(_) => {
                    removed_nodes.push(peer.get_hash_id());
                    None
                }
            };
        }

        for r in removed_nodes {
            self.node_repo.remove(&r);
        }

        self.sessions.read().unwrap().len()
    }

    /// Sends pings and removes dead connections as necessary
    pub fn check_sessions(&mut self) {

        let mut removed: Vec<SocketAddr> = Vec::new();


        let mut s = self.sessions.write().unwrap();

        for (addr, mut sess) in s.iter_mut() {
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
        }

        for remove in removed {
            s.remove(&remove);
        }
    }

    pub fn open_session(&self, peer: Arc<Node>, my_node: Arc<Node>, introduce: Option<&Packet>) -> Result<SocketAddr, ()> {
        let pkh = peer.get_hash_id();
        let saddr = peer.endpoint.clone().as_socketaddr();

        // now we can look into creating a new session
        if let Some(addr) = saddr {
            if self.sessions.read().unwrap().contains_key(&addr) {
                return Err(()); // already connected
            }

            debug!("New session: {:?}", peer.endpoint);

            // ready to connect
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
        queue.push_back(node);
    }

    /// Call to set this shard to a state where all nodes are disconnected and data should stop being validated/tracked
    pub fn close(&self, s: &UdpSocket) {
        debug!("Close shard: {}", self.network_id);
        for mut sess in self.sessions.write().unwrap().values_mut() {
            sess.close();
            send_session_packets(&mut sess, s);
        }
    }

    /// Evaluate a single packet and route it to a session as necessary
    pub fn process_packet(&self, p: &Packet, addr: &SocketAddr, mut context: &mut NetworkContext) {
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
    }

    /// Send all the packets queued for the given session. Returns number of bytes sent.
    pub fn send_packets(&self, addr: &SocketAddr, s: &UdpSocket) -> u64 {
        let mut sr = self.sessions.write().unwrap();
        let mut sess = sr.get_mut(addr).unwrap();

        send_session_packets(&mut sess, s)
    }

    /// Send all the packets queued for any session in this network. Returns number of bytes sent.
    pub fn send_all_packets(&self, s: &UdpSocket) -> usize {
        let mut sent: usize = 0;
        for mut sess in self.sessions.write().unwrap().values_mut() {
            sent += send_session_packets(&mut sess, s) as usize;
        }

        sent
    }

    pub fn get_network_id(&self) -> &U256 {
        return &self.network_id;
    }

    pub fn get_node_repo(&self) -> &NodeRepository {
        return &self.node_repo;
    }

    pub fn session_count(&self) -> usize {
        return self.sessions.read().unwrap().len();
    }
}

fn send_session_packets(sess: &mut Session, s: &UdpSocket) -> u64 {
    let mut count: u64 = 0;

    while let Some(p) = sess.pop_send_queue() {

        let mut rawp = RawPacket {
            port: sess.get_remote().1,
            payload: p
        };

        // TODO: Is it bad that I call this in 2 separate calls, or are they just buffered together?
        let raw = serialize(&rawp, Bounded(Client::MAX_PACKET_SIZE as u64)).unwrap();
        let size_enc = serialize(&(raw.len() as u32), Bounded(4)).unwrap();
        s.send_to(&size_enc[..], sess.get_remote_addr());
        s.send_to(&raw[..], sess.get_remote_addr());

        count += 4 + raw.len() as u64;
    }

    count
}