use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc,Mutex};
use std::cmp::min;

use super::node::{Node, NodeEndpoint};

use u256::*;
use super::block::Block;
use super::super::txn::Txn;
use super::super::time::Time;

use network::client::NetworkContext;

#[derive(Serialize, Deserialize)]
pub struct Packet {
    pub seq: u32,
    pub msg: Message,
}

#[derive(Serialize, Deserialize)]
pub enum DataRequestError {
    /// The requested hash does not exist on this node
    HashNotFound,
    /// Too many requests have come from your node to be processed in quick succession
    RateExceeded,
    /// This node is not an authoritative source for information on the requested shard ID
    NetworkNotAvailable
}

#[derive(Serialize, Deserialize)]
pub enum Message {
    /// First message sent by a connecting node. If the other node accepts, it will reply with an "Introduce". The nodes are now connected
    Introduce {
        /// The network which this node is attempting to make a connection through
        network_id: U256,
        /// Information about the node that is connecting
        node: Node
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

    /// Sent when a previous call to QueryData or SyncBlocks is not able to be fulfilled, in whole or in part.
    DataError {
        err: DataRequestError
    }
}

pub struct Session {

    /// Whether or not we have received Introduce packet from the other end yet. Cannot process any packets otherwise
    introduced: bool,

    /// Set to true when the session is no longer valid, or has completed
    done: bool,

    /// The shard of interest for this session
    network_id: U256,

    /// Information about the node on the other end. If this is unset, then the connection is not really fully initialized yet
    remote_peer: Arc<Node>,

    /// Information about our own node
    local_peer: Arc<Node>,

    /// Latest address information on the remote client (different from NodeEndpoint)
    remote_addr: SocketAddr,

    /// When we first were initialized
    established_since: Time,

    /// Average latency over the last n ping-pong sequences, round trip
    latency: Time,

    /// Time at which the most recent ping packet was sent
    last_ping_send: Option<Time>,

    /// A queue of packets which should be sent to the client soon
    send_queue: Mutex<VecDeque<Packet>>
}

impl Session {

    pub const PROTOCOL_VERSION: u16 = 1;

    /// How often to test ping in seconds
    pub const PING_FREQUENCY: u16 = 30;

    /// How much of the ping value to retain. The current value keeps a weighted average over 10 minutes
    pub const PING_RETENTION: f32 = 40.0;

    /// The number of nodes which should be sent back on a list node request
    pub const NODE_RESPONSE_SIZE: usize = 8;

    pub fn new(local_peer: Arc<Node>, remote_peer: Arc<Node>, remote_addr: SocketAddr, network_id: U256) -> Session {
        let introduce_n = local_peer.as_ref().clone();
        
        let sess = Session {
            introduced: false,
            done: false,
            remote_peer: remote_peer,
            local_peer: local_peer,
            remote_addr: remote_addr,
            network_id: network_id,
            established_since: Time::current(),
            latency:  Time::from_milliseconds(0),
            last_ping_send: None,
            send_queue: Mutex::new(VecDeque::new())
        };

        sess.send_queue.lock().unwrap().push_back(Packet {
            seq: 0,
            msg: Message::Introduce {
                node: introduce_n,
                network_id: network_id
            }
        });

        sess
    }

    pub fn get_remote_node(&self) -> Arc<Node> {
        self.remote_peer.clone()
    }

    pub fn get_remote_addr(&self) -> &SocketAddr {
        &self.remote_addr
    }

    /// Provide a packet which has been received for this session
    pub fn recv(&mut self, packet: &Packet, context: &mut NetworkContext) {

        if !self.introduced {
            // we cannot take this packet
            match packet.msg {
                Message::Introduce { ref node, ref network_id } => {

                    // Update node information in the repository with latest here
                    // It is guarenteed to be in the repo since our remote reference corresponds to it
                    // NOTE: Reference is actually not valid anymore, but we should be OK.
                    self.remote_peer = Arc::new(node.clone());

                    // TODO: Add appendable nodes
                    //context.node_repo.read().unwrap().apply(node);
                },

                _ => {
                    // must receive introduce packet first
                    self.done = true;
                    return;
                }
            }
        }
        else {
            // handle all of the different packet types
            match packet.msg {
                Message::Introduce { ref node, ref network_id } => {
                    // cannot be reintroduced
                    self.done = true;
                    return;
                }

                Message::Ping { ref time } => {
                    // Send back a pong
                    self.send_queue.lock().unwrap().push_back(Packet {
                        seq: 0,
                        msg: Message::Pong {
                            time: time.clone()
                        }
                    });
                },

                Message::Pong { ref time } => {
                    // save ping information
                    if let Some(lps) = self.last_ping_send {
                        if lps == *time {
                            let f = 1.0 / Session::PING_RETENTION;
                            self.latency.apply_weight(&lps.diff(time), f);
                        }

                        self.last_ping_send = None;
                    }
                },

                Message::FindNodes { ref network_id, ref skip } => {

                    // send back a list of nodes that I know about for the specified network
                    let mut nodes: Vec<Node> = Vec::with_capacity(Session::NODE_RESPONSE_SIZE);

                    if let Some(node_repo) = context.node_repo.read().unwrap().get(network_id) {
                        for i in *skip..min(node_repo.len() as u16, (*skip as usize + Session::NODE_RESPONSE_SIZE) as u16) {
                            nodes.push(Arc::try_unwrap(node_repo.get_nodes((skip + i) as usize)).unwrap());
                        }
                    }

                    self.send_queue.lock().unwrap().push_back(Packet {
                        seq: 0,
                        msg: Message::NodeList {
                            nodes: nodes,
                            network_id: network_id.clone(),
                            skip: skip.clone()
                        }
                    });
                },

                Message::NodeList { ref nodes, ref network_id, ref skip } => {
                    // we got back a list of nodes. For right now, we take only the first n of them in order to prevent overflow/whelm
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

                }
            }
        }
    }

    pub fn pop_send_queue(&mut self) -> Option<Packet> {
        return self.send_queue.lock().unwrap().pop_front();
    }
}
