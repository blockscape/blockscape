use super::node::Node;

use super::U256;
use super::Block;
use super::super::txn::Txn;
use super::super::time::Time;

#[derive(Serialize, Deserialize)]
struct Packet {
    msg: Message,
}

#[derive(Serialize, Deserialize)]
enum DataRequestError {
    HashNotFound,
    RateExceeded
}

#[derive(Serialize, Deserialize)]
enum Message {
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
    node: Node,
}

impl Session {
    fn recv(packet: Packet) {
        
    }
}
