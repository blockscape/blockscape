use std::net::SocketAddr;
use super::U160;

/// All the information needed to make contact with a remote node
#[derive(Serialize, Deserialize, Debug)]
pub struct NodeEndpoint {
    /// Network IP of the client
    pub addr: SocketAddr,

    /// Port for UDP communication of node
    pub port: u16,
}

/// Detailed information about a node
#[derive(Serialize, Deserialize)]
pub struct Node {
    /// Information on the address and port which can be used to open a connection to this node
    pub endpoint: NodeEndpoint,
    /// Public key hash derived used for signing messages
    pub key: U160,
    /// The version of the node's network communications. Incremented on any fundamental network change between releases
    pub version: u16
    /// A description for the client, consisting typically of the name of the client, plus the version code
    pub name: String
}

struct LocalNode {
    pub node: Node,
    pub score: u16
}

impl Ord

pub struct NodeRepository {
    available_nodes: HashMap<U160, LocalNode>
}

/// Contains and manages a sorted list of connectable nodes and full information about them
impl NodeRepository {

    /// Based on the local score of nodes, get a list of the best ones to connect to
    /// This is primarily intended for startup, or when there are no nodes connected for whatever reason, and a connection is needed.
    fn getNodeWeighted(self, count: u16) => &Node {
        // choose a random set of nodes from our hashmap
    }

    /// Notify the repository of updated or new node information. Will automatically add or change an existing node as appropriate based on the key in the repository
    fn apply(node: Node) => Result<bool> {
        
    }

    fn upScore(node: U160) {
        
    }

    fn downScore(node: U160) {

    }

    fn save() => Result<u32> {

    }

    fn load() => Result<u32> {

    }
}