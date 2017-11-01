use std::cmp::min;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::{Arc,Mutex};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

use primitives::{Block, Txn, U256, Event};
use super::node::Node;
use time::Time;

use signer::RSA_KEY_SIZE;

use network::client::NetworkContext;


pub const PROTOCOL_VERSION: u16 = 1;

/// How often to test ping in seconds
pub const PING_FREQUENCY: u16 = 30;

/// How much of the ping value to retain. The current value keeps a weighted average over 10 minutes
pub const PING_RETENTION: f32 = 40.0;

/// Number of milliseconds to wait before declaring a ping failed
pub const PING_TIMEOUT: i64 = 3000;

/// The number of strikes which may accumulate before declaring the connection timed out
pub const TIMEOUT_TOLERANCE: u64 = 3;

/// The number of nodes which should be sent back on a list node request
pub const NODE_RESPONSE_SIZE: usize = 8;


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
    HashNotFound,
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
    SyncBlocks { last_block_hash: U256 },
    /// Request specific block or transaction data as indicated by the list of hashes given
    QueryData { hashes: U256 },
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

/// Represents a single connection between another peer on the network.
/// There may be only one session for each peer-network (AKA, a peer could have multiple sessions for separate network_id)
pub struct Session {

    /// Whether or not we have received Introduce packet from the other end yet. Cannot process any packets otherwise
    introduced: bool,

    /// Indicates if the session has completed
    done: Option<ByeReason>,

    /// The shard of interest for this session
    network_id: U256,

    /// Information about the node on the other end. If this is unset, then the connection is not really fully initialized yet
    remote_peer: Arc<Node>,

    /// The port which we connect to on the other end. This starts as 255 for new connections
    remote_port: u8,

    /// Information about our own node
    local_peer: Arc<Node>,

    /// Helper variable for router to manage multiple connections from a single client
    local_port: u8,

    /// Latest address information on the remote client (different from NodeEndpoint)
    remote_addr: SocketAddr,

    /// When we first were initialized
    established_since: Time,

    /// Average latency over the last n ping-pong sequences, round trip
    latency: Time,

    /// Time at which the most recent ping packet was sent
    last_ping_send: Option<Time>,

    /// A queue of packets which should be sent to the client soon
    send_queue: Mutex<VecDeque<Packet>>,

    /// The unique packet identifier to use for the next packet
    current_seq: AtomicUsize,

    /// Used to help discern the number of failed replies. When this number exceeds a threshold,
    /// the connection is considered dropped
    strikes: AtomicUsize,
}

impl Session {
    pub fn new(local_peer: Arc<Node>, local_port: u8, remote_peer: Arc<Node>, remote_addr: SocketAddr, network_id: U256, introduce: Option<&Packet>) -> Session {
        let introduce_n = local_peer.as_ref().clone();
        
        let mut sess = Session {
            local_port: local_port,
            introduced: false,
            done: None,
            remote_peer: remote_peer,
            local_peer: local_peer,
            remote_addr: remote_addr,
            remote_port: 255,
            network_id: network_id,
            established_since: Time::current(),
            latency:  Time::from_milliseconds(0),
            last_ping_send: None,
            send_queue: Mutex::new(VecDeque::new()),
            current_seq: AtomicUsize::new(0),
            strikes: AtomicUsize::new(0),
        };

        if let Some(p) = introduce {
            sess.handle_introduce(&p.msg);
        }

        // connection could have been acquitted while handling the introduce.
        if sess.done.is_none() {
            sess.send_queue.lock().unwrap().push_back(Packet {
                seq: 0,
                msg: Message::Introduce {
                    node: introduce_n,
                    port: local_port,
                    network_id: network_id
                }
            });
        }

        sess
    }

    pub fn get_remote_node(&self) -> &Arc<Node> {
        &self.remote_peer
    }

    pub fn get_remote_addr(&self) -> &SocketAddr {
        &self.remote_addr
    }

    fn handle_introduce(&mut self, msg: &Message) {
        if let &Message::Introduce { ref node, ref network_id, ref port } = msg {
            self.remote_peer = Arc::new(node.clone());
            self.remote_port = *port;

            self.introduced = true;
            self.strikes.store(0, Relaxed);

            //debug!("Remote peer key: {:?}, Local peer key: {:?}", self.remote_peer.key, self.local_peer.key);

            if self.remote_peer.key.len() != self.local_peer.key.len() {
                debug!("Key size is wrong from client: {:?}, expected: {:?}, actual: {:?}", self.remote_peer.endpoint, self.remote_peer.key.len(), RSA_KEY_SIZE);
                self.done = Some(ByeReason::ExitPermanent);
            }

            // detect if we have connected to self
            if self.remote_peer.key == self.local_peer.key {
                debug!("Detected a connection to self, from remote: {:?}", self.remote_peer.endpoint);
                self.done = Some(ByeReason::ExitPermanent);
            }
        }
        else {
            panic!("Received non-introduce packet for session init");
        }
    }

    /// Provide a packet which has been received for this session
    pub fn recv(&mut self, packet: &Packet, context: &mut NetworkContext) {

        if self.done.is_some() {
            return; // no need to do any additional processing
        }

        if !self.introduced {
            // we cannot take this packet
            match packet.msg {
                Message::Introduce { ref node, ref network_id, ref port } => {
                    self.handle_introduce(&packet.msg);
                },

                _ => {
                    // must receive introduce packet first
                    self.done = Some(ByeReason::Exit);
                    return;
                }
            }
        }
        else {
            // handle all of the different packet types
            match packet.msg {
                Message::Introduce { ref node, ref network_id, ref port } => {
                    // cannot be reintroduced
                    // TODO: might not actually be abuse
                    self.done = Some(ByeReason::Abuse);
                    return;
                }

                Message::Ping { ref time } => {
                    // Send back a pong
                    self.send_queue.lock().unwrap().push_back(Packet {
                        seq: packet.seq,
                        msg: Message::Pong {
                            time: time.clone()
                        }
                    });
                },

                Message::Pong { ref time } => {
                    // save ping information
                    if let Some(lps) = self.last_ping_send {
                        if lps == *time {
                            let f = 1.0 / PING_RETENTION;
                            self.latency.apply_weight(&lps.diff(time), f);
                        }

                        self.last_ping_send = None;
                        // now we know the connection is still going, reset strike counter
                        self.strikes.store(0, Relaxed);
                    }
                },

                Message::FindNodes { ref network_id, ref skip } => {

                    // send back a list of nodes that I know about for the specified network
                    let repo = context.load_node_repo(network_id.clone());

                    let mut nodes: Vec<Node> = Vec::with_capacity(min(NODE_RESPONSE_SIZE, repo.len()));

                    for i in *skip..min(repo.len() as u16, (*skip as usize + NODE_RESPONSE_SIZE) as u16) {
                        nodes.push(Arc::try_unwrap(repo.get_nodes((skip + i) as usize)).unwrap());
                    }

                    self.send_queue.lock().unwrap().push_back(Packet {
                        seq: packet.seq,
                        msg: Message::NodeList {
                            nodes: nodes,
                            network_id: network_id.clone(),
                            skip: skip.clone()
                        }
                    });
                },

                Message::NodeList { ref nodes, ref network_id, ref skip } => {
                    // we got back a list of nodes. For right now, we take only the first n of them in order to prevent overflow/whelm
                    if context.connect_peers.contains_key(network_id) {
                        let peers = context.connect_peers.get_mut(network_id).unwrap();
                        peers.extend_from_slice(&nodes[..]);
                    }
                    else {
                        context.connect_peers.insert(network_id.clone(), nodes.clone());
                    }
                },

                Message::NewTransaction { ref txn } => {
                    context.import_txns.push_back(txn.clone());
                },

                Message::NewBlock { ref block } => {
                    context.import_blocks.push_back(block.clone());
                },

                Message::SyncBlocks { ref last_block_hash } => {
                    // get stuff from the db
                },

                Message::QueryData { ref hashes } => {
                    // get stuff form the db
                },

                Message::DataList { ref blocks, ref transactions } => {
                    context.import_txns.extend(transactions.iter().cloned());
                    context.import_blocks.extend(blocks.iter().cloned());
                },

                Message::DataError { ref err } => {

                },

                Message::Bye { ref reason } => {
                    // remote end has closed the connection, no need to reply, just mark this session as that reason
                    self.done = Some((*reason).clone());
                }
            }
        }
    }

    /// Performs checks to verify the current connection state. If the connection appears dead, it will
    /// set this connection as done. Otherwise, it will send a ping packet.
    /// Call this function at regular intervals for best results.
    pub fn check_conn(&mut self) {
        if self.done.is_none() {

            if !self.introduced {
                // we might have to re-send the introduce packet
                let introduce_n = self.local_peer.as_ref().clone();
                self.send_queue.lock().unwrap().push_back(Packet {
                    seq: 0,
                    msg: Message::Introduce {
                        node: introduce_n,
                        port: self.local_port,
                        network_id: self.network_id
                    }
                });

                if self.strikes.fetch_add(1, Relaxed) + 1 > TIMEOUT_TOLERANCE as usize {
                    self.done = Some(ByeReason::Timeout);
                }
            }
            else {
                // if we still have an outgoing ping and too much time has passed, add a strike
                if let Some(lps) = self.last_ping_send {
                    if lps.diff(&Time::current()).millis() > PING_TIMEOUT {
                        self.strikes.fetch_add(1, Relaxed);
                    }
                }

                //debug!("Connection Strikes: {}", self.strikes.load(Relaxed));

                if self.strikes.load(Relaxed) > TIMEOUT_TOLERANCE as usize {
                    self.done = Some(ByeReason::Timeout);
                }
                else {

                    let lps = Time::current();

                    self.send_queue.lock().unwrap().push_back(Packet {
                        seq: self.current_seq.fetch_add(1, Relaxed) as u32,
                        msg: Message::Ping {
                            time: lps
                        }
                    });

                    self.last_ping_send = Some(lps);
                }
            }
        }
    }

    pub fn find_nodes(&self, network_id: &U256) {
        if self.introduced {
            self.send_queue.lock().unwrap().push_back(Packet {
                seq: self.current_seq.fetch_add(1, Relaxed) as u32,
                msg: Message::FindNodes {
                    network_id: network_id.clone(),
                    skip: 0
                }
            });
        }
    }

    /// Appends a bye packet to the end of the queue
    /// NOTE: Dont forget to empty the send queue after calling this function!
    pub fn close(&mut self) {
        self.send_queue.lock().unwrap().push_back(Packet {
            seq: self.current_seq.fetch_add(1, Relaxed) as u32,
            msg: Message::Bye { reason: ByeReason::Exit }
        });

        self.done = Some(ByeReason::Exit);
    }

    pub fn pop_send_queue(&mut self) -> Option<Packet> {
        self.send_queue.lock().unwrap().pop_front()
    }

    pub fn get_remote(&self) -> (&SocketAddr, u8) {
        (&self.remote_addr, self.remote_port)
    }

    pub fn is_done(&self) -> Option<ByeReason> {
        self.done
    }

    pub fn is_introduced(&self) -> bool {
        self.introduced
    }
}
