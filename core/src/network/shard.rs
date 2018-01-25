use std::cell::*;
use std::cmp::min;
use rand;
use rand::Rng;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::ops::Deref;
use std::rc::Rc;

use network::client::{ShardMode, NetworkActions};
use network::context::*;
use network::node::{Node, NodeRepository};
use network::session::{Session, Message, SessionInfo, ByeReason, Packet};
use primitives::{U256, U160};

pub struct ShardInfo {

    /// The network context
    context: Rc<NetworkContext>,

    /// Unique identifier for this shard (usually genesis block hash)
    network_id: U256,

    /// The port which should be assigned to clients
    pub port: u8,

    /// Functional requirements of this shard connection
    pub mode: ShardMode,

    /// Independant "connections" to each of the other NodeEndpoints we interact with
    sessions: RefCell<HashMap<SocketAddr, Session>>,

    peer_ids: RefCell<HashSet<U160>>,

    connect_queue: RefCell<HashSet<Arc<Node>>>,

    /// The index of the node we should scan next in the node repository. Incremented for each connection attempt
    last_peer_idx: Cell<usize>,

    /// Collection of nodes which can be connected to this on this network
    node_repo: RefCell<NodeRepository>
}

impl ShardInfo {
    pub fn new(network_id: U256, port: u8, mode: ShardMode, context: Rc<NetworkContext>, repo: NodeRepository) -> ShardInfo {
        ShardInfo {
            context: context,
            network_id: network_id,
            port: port,
            mode: mode,
            sessions: RefCell::new(HashMap::new()),
            peer_ids: RefCell::new(HashSet::new()),
            last_peer_idx: Cell::new(0),
            node_repo: RefCell::new(repo),
            connect_queue: RefCell::new(HashSet::new())
        }
    }

    /// Try to scan the node database and ensure the minimum requested number of nodes
    pub fn node_scan(&self, wanted: usize, actions: &mut NetworkActions) -> usize {

        let cur_count = self.sessions.borrow().len();

        if cur_count >= wanted {
            return cur_count; // no need to do any more
        }

        // Ask a random peer for more nodes, to keep the database saturated
        {
            let s = self.sessions.borrow();
            if !s.is_empty() {
                let mut rng = rand::thread_rng();
                let sess = rand::sample(&mut rng, s.values(), 1)[0];

                sess.find_nodes(&self.network_id, actions);
            }
        }

        let mut removed_nodes: Vec<U160> = Vec::new();
        {
            let mut queue = self.connect_queue.borrow_mut();

            let nrepo = self.node_repo.borrow();

            //info!("Starting new node queue size: {}, repo size: {}", queue.len(), nrepo.len());

            // do we need more nodes to connect to (from the queue)? If so, pull from the node repo
            let mut attempts = 0;
            if nrepo.len() > 0 {
                while cur_count + queue.len() < wanted {
                    let peer = nrepo.get_nodes(self.last_peer_idx.get());
                    self.last_peer_idx.set(self.last_peer_idx.get() + 1);

                    if !self.peer_ids.borrow().contains(&peer.get_hash_id()) && peer.get_hash_id() != self.context.my_node.get_hash_id() {
                        queue.insert(peer);
                    }

                    attempts += 1;

                    if attempts >= wanted * 3 || attempts >= nrepo.len() {
                        break;
                    }
                }
            }

            debug!("Reaching for {} new nodes...", queue.len());

            // pull from the connection queue

            for peer in queue.drain() {
                match self.open_session(peer.clone(), None, actions) {
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
        }

        if !removed_nodes.is_empty() {
            let mut nrepo = self.node_repo.borrow_mut();
            for r in removed_nodes {
                debug!("Remove broken node from repo: {:?}", r);
                nrepo.remove(&r);
            }

            let r = nrepo.save(format!("{}", self.network_id).as_str());

            if r.is_err() {
                warn!("Failed to save nodes to file: {:?}", r.unwrap_err());
            }
            else {
                debug!("Saved {} nodes from repo", r.unwrap());
            }
        }

        self.sessions.borrow().len()
    }

    /// Sends pings and removes dead connections as necessary
    pub fn check_sessions(&self, actions: &mut NetworkActions) {

        let mut removed: Vec<SocketAddr> = Vec::new();


        let mut s = self.sessions.borrow_mut();
        let mut pids = self.peer_ids.borrow_mut();

        pids.clear();

        for (addr, sess) in s.iter_mut() {
            sess.check_conn(actions);
            sess.check_job(actions);

            if let Some(d) = sess.is_done() {

                debug!("Remove session: {:?}", sess.get_remote_node().endpoint);
                removed.push(addr.clone());

                // may have to do something additional depending on the failure reason:
                match d {
                    ByeReason::ExitPermanent => {
                        // remove this node from the db as well
                        self.node_repo.borrow_mut().remove(
                            &sess.get_remote_node().get_hash_id()
                        );
                    },
                    // TODO: Maybe also add the node to a blacklist of some kind?
                    ByeReason::Abuse => {
                        // remove this node from the db as well
                        self.node_repo.borrow_mut().remove(
                            &sess.get_remote_node().get_hash_id()
                        );
                    },
                    _ => {}
                }
            }
            else {
                pids.insert(sess.get_remote_node().get_hash_id());
            }

            // TODO: for now this is a little inefficient (requires a U160 hash for each client every 5 seconds), but it works
            // add introduced nodes to the repo
            if sess.is_introduced() && self.node_repo.borrow().get(&sess.get_remote_node().get_hash_id()).is_none() {
                debug!("Add node to DB: {:?}", sess.get_remote_node().get_hash_id());
                self.node_repo.borrow_mut().new_node(
                    sess.get_remote_node().deref().as_ref().clone()
                );
            }
        }

        for remove in removed {
            s.remove(&remove);
        }
    }

    pub fn open_session(&self, peer: Arc<Node>, introduce: Option<&Packet>, actions: &mut NetworkActions) -> Result<SocketAddr, ()> {
        
        // filter out obvious failure cases:
        // connect to self?
        if peer.endpoint == self.context.my_node.endpoint {
            return Err(());
        }
        
        let saddr = peer.endpoint.clone().as_socketaddr();

        // now we can look into creating a new session
        if let Some(addr) = saddr {

            if self.sessions.borrow().contains_key(&addr) {
                return Err(()); // already connected
            }

            //debug!("New session: {:?}", peer.endpoint);

            // readyf to connect
            let sess = Session::new(Rc::clone(&self.context), self.port, peer, addr, self.network_id.clone(), introduce, actions);

            self.peer_ids.borrow_mut().insert(sess.get_remote_node().get_hash_id());
            self.sessions.borrow_mut().insert(addr, sess);

            Ok(addr)
        }
        else {
            // We have bogus data
            warn!("Could not resolve hostname for node: {:?}", peer.endpoint);
            Err(())
        }
    }

    pub fn add_connect_queue(&self, node: Arc<Node>) {

        if !self.peer_ids.borrow().contains(&node.get_hash_id()) && node.get_hash_id() != self.context.my_node.get_hash_id() {
            self.connect_queue.borrow_mut().insert(node);
        }
    }

    /// Try to give the provided job to a randomly selected node in the network
    pub fn assign_job(&self, job: &Rc<NetworkJob>) -> bool {
        let s = self.sessions.borrow();
        let mut rng = rand::thread_rng();
        let mut pulls: Vec<&Session> = s.values().collect();
        rng.shuffle(&mut pulls);

        for pull in pulls {
            if pull.assign_job(job) {
                return true;
            }
        }

        false
    }

    /// Call to set this shard to a state where all nodes are disconnected and data should stop being validated/tracked
    pub fn close(&self, actions: &mut NetworkActions) {
        debug!("Close shard: {}", self.network_id);
        for sess in self.sessions.borrow_mut().values_mut() {
            sess.close(actions);
        }
    }

    /// Evaluate a single packet and route it to a session as necessary
    pub fn process_packet(&self, p: &Packet, addr: &SocketAddr, actions: &mut NetworkActions) {
        
        // debug logging
        match p.msg {
            Message::Ping { .. } => {},
            Message::Pong { .. } => {},
            Message::Introduce { ref node, .. } => debug!("{} ==> Introduce {}", addr, node.get_hash_id()),
            Message::NodeList { ref nodes, .. } => debug!("Received NodeList of {} nodes", nodes.len()),
            _ => debug!("{} ==> {:?}", addr, &p)
        };

        if let Some(sess) = self.sessions.borrow().get(&addr) {
            sess.recv(&p, actions);
        }
        else {
            // special case can happen if UDP packet routing leads to a narrower connection path
            if let Message::Introduce {ref node, ..} = p.msg {
                let hid = node.get_hash_id();
                
                // find it
                let mut sessions = self.sessions.borrow_mut();
                let mut rekey = None;

                if let Some((key, val)) = sessions.iter_mut().find(|&(_,ref v)| v.get_remote_node().endpoint == node.endpoint) {
                    if !val.update_introduce(p, addr) {
                        warn!("Apparent hijack attempt, or simply incompetant host detected");
                    }
                    else {
                        info!("Remote peer {} has changed connection configuration!", hid);
                        rekey = Some(*key);
                    }
                }
                else {
                    // could not find
                    warn!("Unroutable introduce packet: {:?}", node.get_hash_id());
                }
                
                if let Some(k) = rekey {
                    let tmp = sessions.remove(&k).unwrap();
                    sessions.insert(*addr, tmp);
                }
            }
            else {
                // should never happen because all sessions init through port 255
                warn!("Unroutable packet: {:?}", p);
            }
        }
    }

    pub fn get_network_id(&self) -> &U256 {
        return &self.network_id;
    }

    pub fn session_count(&self) -> usize {
        // filter only sessions which are past introductions
        let mut count = 0;
        for sess in self.sessions.borrow().values() {
            if sess.is_introduced() {
                count += 1;
            }
        }

        count
    }

    /// Returns information on all active sessions
    pub fn get_session_info(&self) -> Vec<SessionInfo> {

        let mut vec = Vec::new();

        for sess in self.sessions.borrow().values() {
            if sess.is_introduced() {
                vec.push(sess.get_info());
            }
        }

        vec
    }

    pub fn get_nodes_from_repo(&self, skip: usize, count: usize) -> Vec<Node> {

        let nrepo = self.node_repo.borrow();

        let mut nodes: Vec<Node> = Vec::with_capacity(min(count, nrepo.len()));

        for i in skip..min(nrepo.len(), skip + count) {
            // dont send the node if it is self
            let d = nrepo.get_nodes((skip + i) as usize);
            let n = d.deref();

            nodes.push(n.clone());
        }

        nodes
    }
}