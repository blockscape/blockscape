use serde::Serialize;
use serde::Deserialize;

use std::net::SocketAddr;

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
    pub endpoint: NodeEndpoint,
}