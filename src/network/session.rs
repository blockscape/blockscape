use std::net::UdpSocket;


#[derive(Serialize, Deserialize)]
struct Packet {
    msg: Message
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

    SyncBlocks { lastBlockHash: u256 },
    
    BlockList { blocks: Block }
}

pub struct Session {
    node: Node
}

impl Session {
    fn recv(packet: Packet) {

    }
}