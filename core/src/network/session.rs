use std::collections::linked_list::LinkedList;
use std::net::SocketAddr;
use std::sync::Arc;

use super::node::{Node, NodeEndpoint};

use super::U256;
use super::U160;
use super::block::Block;
use super::super::txn::Txn;
use super::super::time::Time;

use network::client::Client;

#[derive(Serialize, Deserialize)]
pub struct Packet {
    pub seq: u32,
    pub msg: Message,
}

#[derive(Serialize, Deserialize)]
enum DataRequestError {
    HashNotFound,
    RateExceeded
}

#[derive(Serialize, Deserialize)]
pub enum Message {
    /// First message sent by a connecting node. If the other node accepts, it will reply with an "Introduce". The nodes are now connected
    Introduce { node: Node },                   

    /// Sent to check connection status with client
    Ping { time: Time },
    /// Sent to reply to a previous connection status request
    Pong { time: Time },

    /// Sent when a node would like to query peers of another node, in order to form more connections to the network
    FindNodes { network_id: U256 },
    /// In reply to FindNodes, to indicate nodes which can be connected to
    NodeList { nodes: Vec<Node> },

    /// Sent by reliable flooding to indicate a new transaction has entered the network and should be propogated
    NewTransaction { txn: Txn },
    /// Sent by reliable flooding to indicate that a new block has entered the network and should be propogated
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

    /// A queue of packets which should be sent to the client soon
    send_queue: LinkedList<Packet>
}

impl Session {

    pub const PROTOCOL_VERSION: u16 = 1;

    pub fn new(local_peer: Arc<Node>, remote_peer: Arc<Node>, remote_addr: SocketAddr) -> Session {
        let introduce_n = local_peer.as_ref().clone();
        
        let mut sess = Session {
            remote_peer: remote_peer,
            local_peer: local_peer,
            remote_addr: remote_addr,
            established_since: Time::current(),
            latency:  Time::from_milliseconds(0),
            send_queue: LinkedList::new()
        };

        sess.send_queue.push_back(Packet {
            seq: 0,
            msg: Message::Introduce {
                node: introduce_n
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
    pub fn recv(&mut self, packet: &Packet) {
        // handle all of the different packet types
        match packet.msg {
            Message::Introduce { ref node } => {

            },

            Message::Ping { ref time } => {

            },

            Message::Pong { ref time } => {

            },

            Message::FindNodes { ref network_id } => {
                // send back a list of nodes that I know about
            },

            Message::NodeList { ref nodes } => {

            },

            Message::NewTransaction { ref txn } => {

            },

            Message::NewBlock { ref block } => {

            },

            Message::SyncBlocks { ref last_block_hash } => {

            },

            Message::QueryData { ref hashes } => {

            },

            Message::DataList { ref blocks, ref transactions } => {

            },

            Message::DataError { ref err } => {

            }
        }
    }

    pub fn pop_send_queue(&mut self) -> Option<Packet> {
        return self.send_queue.pop_front();
    }
}
