use std::cell::*;
use std::net::SocketAddr;
use std::sync::Arc;
use std::rc::Rc;

use primitives::{Block, Txn, U256};
use super::node::Node;
use time::Time;

use signer::RSA_KEY_SIZE;

use network::context::NetworkContext;
use network::client::NetworkActions;

use futures::prelude::*;

pub const PROTOCOL_VERSION: u16 = 1;

/// How much of the ping value to retain. The current value keeps a weighted average over 10 minutes
pub const PING_RETENTION: f32 = 40.0;

/// Number of milliseconds to wait before declaring a ping failed
pub const PING_TIMEOUT: i64 = 3000;

/// The number of strikes which may accumulate before declaring the connection timed out
pub const TIMEOUT_TOLERANCE: u64 = 3;

/// The number of nodes which should be sent back on a list node request
pub const NODE_RESPONSE_SIZE: usize = 8;

pub struct SocketPacket(pub SocketAddr, pub RawPacket);

#[derive(Serialize, Deserialize, Debug)]
pub struct RawPacket {
    /// Which communication channel should be regarded for this node.
    /// This is included so nodes can have multiple connections to each other through separate shards
    /// Port 255 is reserved for connecting from remote nodes when the local port is unknown
    pub port: u8,
    /// The data which should be delivered to the session handler
    pub payload: Packet
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub seq: u32,
    pub msg: Message,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DataRequestError {
    /// The requested hash does not exist on this node
    HashesNotFound(Vec<U256>),
    /// Too many requests have come from your node to be processed in quick succession
    RateExceeded,
    /// This node is not an authoritative source for information on the requested shard ID
    NetworkNotAvailable
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum ByeReason {
    /// Node is simply disconnecting
    Exit,
    /// Connection should not be attempted to this node again because the node is incompatible or wrong
    /// For example, connecting to self.
    ExitPermanent,
    /// Node has not been responding, or no longer seems to be available
    Timeout,
    /// Node has not been nice, so disconnect
    Abuse
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    /// First message sent by a connecting node. If the other node accepts, it will reply with an "Introduce". The nodes are now connected
    Introduce {
        /// The network which this node is attempting to make a connection through
        network_id: U256,
        /// Information about the node that is connecting
        node: Node,
        /// The port which should be used for future packets to this node for this network
        port: u8
    },                   

    /// Sent to check connection status with client
    Ping {
        /// The time at which the ping started
        time: Time
    },
    /// Sent to reply to a previous connection status request
    Pong {
        /// The time at which the ping started
        time: Time
    },

    /// Sent when a node would like to query peers of another node, in order to form more connections to the network
    FindNodes {
        /// Regardless of whatever network ID may be associated with a session, this property defines which network to return packets of
        network_id: U256,
        /// If more nodes are needed, an offset can be indicated here to get additional nodes with subsequent queries
        skip: u16
    },

    /// In reply to FindNodes, to indicate nodes which can be connected to
    NodeList {
        /// A list of nodes which can be connected to. An empty list means there is no more data here
        nodes: Vec<Node>,
        /// The original requested network id
        network_id: U256,
        /// If more nodes are needed, an offset can be indicated here to get additional nodes with subsequent queries (just increment by however many you received)
        /// Currently this is not used for anything.
        skip: u16
    },

    /// Sent by reliable flooding to indicate a new transaction has entered the network and should be propogated
    NewTransaction { txn: Txn },
    /// Sent by reliable flooding to indicClientate that a new block has entered the network and should be propogated
    NewBlock { block: Block },

    /// Request block synchronization data, starting from the given block hash, proceeding to the last block hash
    SyncBlocks { last_block_hash: U256, target_block_hash: U256 },
    /// Request specific block or transaction data as indicated by the list of hashes given
    QueryData { hashes: Vec<U256> },
    /// Returned in response to a request for txn/block data (either SyncBlocks or QueryData) to provide bulk data to import from the blockchain
    DataList {
        /// A list of blocks to import
        blocks: Vec<Block>,    
        /// A list of transactions to import                 
        transactions: Vec<Txn>,
    },

    /// Sent to signal the end of the connection
    Bye {
        /// Why the connection was closed
        reason: ByeReason
    },

    /// Sent when a previous call to QueryData or SyncBlocks is not able to be fulfilled, in whole or in part.
    DataError {
        err: DataRequestError
    }
}

/// Statistical information object for detailed information about a session
#[derive(Serialize, Deserialize, Debug)]
pub struct SessionInfo {
    network_id: U256,
    peer: Node,
    latency: Time,
    established_since: Time
}

/// Represents a single connection between another peer on the network.
/// There may be only one session for each peer-network (AKA, a peer could have multiple sessions for separate network_id)
pub struct Session {

    context: Rc<NetworkContext>,

    /// Indicates if the session has completed
    done: Cell<Option<ByeReason>>,

    /// The shard of interest for this session
    network_id: U256,

    /// Information about the node on the other end. If this is unset, then the connection is not really fully initialized yet
    remote_peer: RefCell<Arc<Node>>,

    /// The port which we connect to on the other end. This starts as 255 for new connections
    remote_port: Cell<u8>,

    /// Helper variable for router to manage multiple connections from a single client
    local_port: u8,

    /// Latest address information on the remote client (different from NodeEndpoint)
    remote_addr: SocketAddr,

    /// When we first were initialized
    established_since: Time,

    /// Average latency over the last n ping-pong sequences, round trip
    latency: Cell<Time>,

    /// Time at which the most recent ping packet was sent
    last_ping_send: Cell<Option<Time>>,

    /// The unique packet identifier to use for the next packet
    current_seq: Cell<u32>,

    /// Used to help discern the number of failed replies. When this number exceeds a threshold,
    /// the connection is considered dropped
    strikes: Cell<u32>,
}

impl Session {
    pub fn new(context: Rc<NetworkContext>, local_port: u8, remote_peer: Arc<Node>, remote_addr: SocketAddr, network_id: U256, introduce: Option<&Packet>, actions: &mut NetworkActions) -> Session {

        let sess = Session {
            context: context,
            local_port: local_port,
            done: Cell::new(None),
            remote_peer: RefCell::new(remote_peer),
            remote_addr: remote_addr,
            remote_port: Cell::new(255),
            network_id: network_id,
            established_since: Time::current(),
            latency:  Cell::new(Time::from_milliseconds(0)),
            last_ping_send: Cell::new(None),
            current_seq: Cell::new(0),
            strikes: Cell::new(0),
        };

        if let Some(p) = introduce {
            sess.handle_introduce(&p.msg);
        }

        // connection could have been acquitted while handling the introduce.
        if sess.done.get().is_none() {
            actions.send_packets.push(SocketPacket(sess.remote_addr.clone(), RawPacket {
                port: sess.remote_port.get(),
                payload: Packet {
                    seq: 0,
                    msg: Message::Introduce {
                        node: sess.context.my_node.clone(),
                        port: local_port,
                        network_id: network_id
                    }
            }}));
        }

        sess
    }

    pub fn get_remote_node(&self) -> Ref<Arc<Node>> {
        // pulling an arc out of a cell basically requires two swaps
        self.remote_peer.borrow()
    }

    /*pub fn get_remote_addr(&self) -> &SocketAddr {
        &self.remote_addr
    }*/

    fn handle_introduce(&self, msg: &Message) {
        if let &Message::Introduce { ref node, ref port, .. } = msg {
            *self.remote_peer.borrow_mut() = Arc::new(node.clone());

            self.remote_port.set(*port);

            self.strikes.set(0);

            if self.remote_peer.borrow().key.len() != self.context.my_node.key.len() {
                debug!("Key size is wrong from client: {:?}, expected: {:?}, actual: {:?}", node.endpoint, node.key.len(), RSA_KEY_SIZE);
                self.done.set(Some(ByeReason::ExitPermanent));
            }

            // detect if we have connected to self
            if node.key == self.context.my_node.key {
                debug!("Detected a connection to self, from remote: {:?}", self.remote_peer.borrow().endpoint);
                self.done.set(Some(ByeReason::ExitPermanent));
            }
        }
        else {
            panic!("Received non-introduce packet for session init");
        }
    }

    /// Provide a packet which has been received for this session
    pub fn recv(&self, packet: &Packet, actions: &mut NetworkActions) {

        if self.done.get().is_some() {
            return; // no need to do any additional processing
        }

        if !self.is_introduced() {
            // we cannot take this packet
            match packet.msg {
                Message::Introduce { .. } => {
                    self.handle_introduce(&packet.msg);
                },

                _ => {
                    // must receive introduce packet first
                    self.done.set(Some(ByeReason::Exit));
                    return;
                }
            }
        }
        else {
            // handle all of the different packet types
            match packet.msg {
                Message::Introduce { .. } => {
                    // cannot be reintroduced
                    // TODO: might not actually be abuse
                    self.done.set(Some(ByeReason::Abuse));
                    return;
                }

                Message::Ping { ref time } => {
                    // Send back a pong
                    actions.send_packets.push(SocketPacket(self.remote_addr.clone(), RawPacket {
                        port: self.remote_port.get(),
                        payload: Packet {
                            seq: packet.seq,
                            msg: Message::Pong {
                                time: time.clone()
                            }
                    }}));
                },

                Message::Pong { ref time } => {
                    // save ping information
                    if let Some(lps) = self.last_ping_send.get() {
                        if lps == *time {
                            let f = 1.0 / PING_RETENTION;
                            let mut l = self.latency.get();
                            l.apply_weight(&lps.diff(time), f);
                            self.latency.set(l);
                        }

                        self.last_ping_send.set(None);
                        // now we know the connection is still going, reset strike counter
                        self.strikes.set(0);
                    }
                },

                Message::FindNodes { ref network_id, ref skip } => {

                    let nodes = actions.nc.get_shard_peer_info(network_id).into_iter()
                        .skip(*skip as usize)
                        .take(NODE_RESPONSE_SIZE as usize)
                        .filter_map(|p| {
                            if &p.peer == (*self.remote_peer.borrow()).as_ref() {
                                None
                            }
                            else {
                                Some(p.peer)
                            }
                        })
                        .collect();

                    /*let nodes = if *network_id == self.network_id {
                        actions.shard.get_nodes_from_repo(*skip as usize, NODE_RESPONSE_SIZE as usize)
                    }
                    else {
                        actions.nc.get_nodes_from_repo(network_id, *skip as usize, NODE_RESPONSE_SIZE as usize)
                    };*/

                    actions.send_packets.push(SocketPacket(self.remote_addr.clone(), RawPacket {
                        port: self.remote_port.get(),
                        payload: Packet {
                            seq: packet.seq,
                            msg: Message::NodeList {
                                nodes: nodes,
                                network_id: network_id.clone(),
                                skip: skip.clone()
                            }
                    }}));
                },

                Message::NodeList { ref nodes, ref network_id, .. } => {
                    // we got back a list of nodes. For right now, we take only the first n of them in order to prevent overflow/whelm
                    if actions.connect_peers.contains_key(network_id) {
                        let peers = actions.connect_peers.get_mut(network_id).unwrap();
                        peers.extend_from_slice(&nodes[..]);
                    }
                    else {
                        actions.connect_peers.insert(network_id.clone(), nodes.clone());
                    }
                },

                Message::NewTransaction { ref txn } => {
                    let d = txn.clone();
                    let rk = Arc::clone(&self.context.rk);
                    self.context.event_loop.spawn(self.context.rk.get_worker().spawn_fn(move || {
                        rk.add_pending_txn(&d)
                    }).map(|_| ()).or_else(|_| {
                        // react for this node's records here if they are bad
                        Ok::<(), ()>(())
                    }));
                },

                Message::NewBlock { ref block } => {
                    let d = block.clone();
                    let rk = Arc::clone(&self.context.rk);
                    self.context.event_loop.spawn(self.context.rk.get_worker().spawn_fn(move || {
                        rk.add_block(&d)
                    }).map(|_| ()).or_else(|_| {
                        // react for this node's records here if they are bad
                        Ok::<(), ()>(())
                    }));
                },

                Message::SyncBlocks { /*ref last_block_hash, ref target_block_hash*/ .. } => {
                    // get stuff from the db

                },

                Message::QueryData { ref hashes } => {
                    let d = hashes.clone();
                    let r_addr = self.remote_addr.clone();
                    let r_port = self.remote_port.get();
                    let lcontext = Rc::clone(&self.context);
                    let seq = packet.seq;
                    // get stuff form the db
                    let rk = Arc::clone(&self.context.rk);
                    self.context.event_loop.spawn(self.context.rk.get_worker().spawn_fn(move || {
                        let mut blocks: Vec<Block> = Vec::new();
                        let mut txns: Vec<Txn> = Vec::new();

                        let mut failed: Vec<U256> = Vec::new();



                        for hash in d {
                            if let Ok(txn) = rk.get_txn(&hash) {
                                txns.push(txn);
                            }
                            else if let Ok(block) = rk.get_block(&hash) {
                                blocks.push(block);
                            }
                            else {
                                failed.push(hash.clone());
                            }
                        }

                        Ok((blocks, txns, failed))

                    }).and_then(move |(blocks, txns, failed)| {

                        let mut to_send = Vec::with_capacity(2);

                        if !blocks.is_empty() || !txns.is_empty() {
                            to_send.push(SocketPacket(r_addr.clone(), RawPacket {
                                port: r_port,
                                payload: Packet {
                                    seq: seq,
                                    msg: Message::DataList {
                                        blocks: blocks,
                                        transactions: txns
                                    }
                            }}));
                        }
                        if !failed.is_empty() {
                            to_send.push(SocketPacket(r_addr, RawPacket {
                                port: r_port,
                                payload: Packet {
                                    seq: seq,
                                    msg: Message::DataError {
                                        err: DataRequestError::HashesNotFound(failed)
                                    }
                            }}));
                        }

                        lcontext.send_packets(to_send);
                        Ok::<(), ()>(())
                    }));
                },

                Message::DataList { .. } => {
                    let f1 = self.context.rk.get_worker().spawn_fn(|| {
                        // import block package
                        Ok::<(), ()>(())
                    });

                    let f2 = f1.or_else(|_| {
                        // react for this node's records here if they are bad
                        Ok::<(), ()>(())
                    });

                    self.context.event_loop.spawn(f2);
                },

                Message::DataError { .. } => {

                },

                Message::Bye { ref reason } => {
                    // remote end has closed the connection, no need to reply, just mark this session as that reason
                    self.done.set(Some(reason.clone()));
                }
            }
        }
    }

    /// Performs checks to verify the current connection state. If the connection appears dead, it will
    /// set this connection as done. Otherwise, it will send a ping packet.
    /// Call this function at regular intervals for best results.
    pub fn check_conn(&self, actions: &mut NetworkActions) {
        if self.done.get().is_none() {

            if !self.is_introduced() {
                // we might have to re-send the introduce packet
                let introduce_n = self.context.my_node.clone();
                actions.send_packets.push(SocketPacket(self.remote_addr.clone(), RawPacket {
                    port: self.remote_port.get(),
                    payload: Packet {
                        seq: 0,
                        msg: Message::Introduce {
                            node: introduce_n,
                            port: self.local_port,
                            network_id: self.network_id
                        }
                }}));

                if self.strikes.replace(self.strikes.get() + 1) + 1 > TIMEOUT_TOLERANCE as u32 {
                    self.done.set(Some(ByeReason::Timeout));
                }
            }
            else {
                // if we still have an outgoing ping and too much time has passed, add a strike
                if let Some(lps) = self.last_ping_send.get() {
                    if lps.diff(&Time::current()).millis() > PING_TIMEOUT {
                        self.strikes.set(self.strikes.get() + 1);
                    }
                }

                //debug!("Connection Strikes: {}", self.strikes.load(Relaxed));

                if self.strikes.get() > TIMEOUT_TOLERANCE as u32 {
                    self.done.set(Some(ByeReason::Timeout));
                }
                else {

                    let lps = Time::current();

                    actions.send_packets.push(SocketPacket(self.remote_addr.clone(), RawPacket {
                        port: self.remote_port.get(),
                        payload: Packet {
                            seq: self.current_seq.replace(self.current_seq.get()),
                            msg: Message::Ping {
                                time: lps
                            }
                    }}));

                    self.last_ping_send.set(Some(lps));
                }
            }
        }
    }

    pub fn find_nodes(&self, network_id: &U256, actions: &mut NetworkActions) {
        if self.is_introduced() {
            actions.send_packets.push(SocketPacket(self.remote_addr.clone(), RawPacket {
                port: self.remote_port.get(),
                payload: Packet {
                    seq: self.current_seq.replace(self.current_seq.get() + 1),
                    msg: Message::FindNodes {
                        network_id: network_id.clone(),
                        skip: 0
                    }
            }}));
        }
    }

    pub fn get_info(&self) -> SessionInfo {
        SessionInfo {
            peer: self.remote_peer.borrow().as_ref().clone(),
            network_id: self.network_id,
            latency: self.latency.get(),
            established_since: self.established_since
        }
    }

    /// Appends a bye packet to the end of the queue
    /// NOTE: Dont forget to empty the send queue after calling this function!
    pub fn close(&self, actions: &mut NetworkActions) {
        actions.send_packets.push(SocketPacket(self.remote_addr.clone(), RawPacket {
            port: self.remote_port.get(),
            payload: Packet {
                seq: self.current_seq.replace(self.current_seq.get() + 1 as u32),
                msg: Message::Bye { reason: ByeReason::Exit }
        }}));

        self.done.set(Some(ByeReason::Exit));
    }

    /*pub fn get_remote(&self) -> (&SocketAddr, u8) {
        (&self.remote_addr, self.remote_port.get())
    }*/

    pub fn is_done(&self) -> Option<ByeReason> {
        self.done.get()
    }

    pub fn is_introduced(&self) -> bool {
        self.remote_port.get() != 255
    }
}
