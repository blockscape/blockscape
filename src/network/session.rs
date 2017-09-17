use std::net::UdpSocket;

use super::node::Node;

use super::U256;
use super::Block;
use super::super::transaction::Transaction;
use super::super::time::Time;

#[derive(Serialize, Deserialize)]
struct Packet {
    msg: Message,
}

#[derive(Serialize, Deserialize)]
enum Message {
    Introduce { node: Node },

    Ping { time: Time },
    Pong { time: Time },

    FindNodes {},
    NodeList { nodes: Vec<Node> },

    NewTransaction { txn: Transaction },
    NewBlock { block: Block },

    SyncBlocks { last_block_hash: U256 },
    QueryData { hashes: U256 },
    DataList {
        blocks: Vec<Block>,
        transactions: Vec<Transaction>,
    },
}

pub struct Session {
    socket: UdpSocket,
    node: Node,
}

impl Session {
    fn recv(packet: Packet) {
        
    }
}
