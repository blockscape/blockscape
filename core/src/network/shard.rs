use std::collections::HashMap;
use std::net::{SocketAddr,UdpSocket};
use std::sync::RwLock;
use std::sync::Arc;

use bincode::{serialize, deserialize, Bounded};

use u160::*;
use u256::*;

use block::*;
use txn::*;
use hash::*;
use time::Time;

use network::client::*;
use network::session::*;
use network::node::*;
use network::ntp::*;

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