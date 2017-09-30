use std::net::SocketAddr;
use super::U160;

use std::collections::HashMap;
use std::fs::File;
use super::env::get_storage_dir;
use serde_json;

use std::path::*;
use std::cmp::*;

use std::io::{Read, Write, Error};

use rand;

/// All the information needed to make contact with a remote node
#[derive(Clone, Serialize, PartialEq, Eq, Deserialize, Debug)]
pub struct NodeEndpoint {
    /// Network IP of the client
    pub host: String,

    /// Port for UDP communication of node
    pub port: u16,
}

/// Detailed information about a node
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Information on the address and port which can be used to open a connection to this node
    pub endpoint: NodeEndpoint,
    /// Public key hash derived used for signing messages
    pub key: U160,
    /// The version of the node's network communications. Incremented on any fundamental network change between releases
    pub version: u16,
    /// A description for the client, consisting typically of the name of the client, plus the version code
    pub name: String
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize)]
struct LocalNode {
    pub node: Node,
    pub score: u16
}

impl LocalNode {
    pub fn new(node: Node) -> LocalNode {
        LocalNode {
            node: node,
            score: 0
        }
    }
}

impl Ord for LocalNode {
    /// Calculate the order between this object and another.
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.score.cmp(&rhs.score)
    }
}

impl PartialOrd for LocalNode {
    /// Calculate the order between this object and another.
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        self.score.partial_cmp(&rhs.score)
    }
}

pub struct NodeRepository {
    available_nodes: HashMap<U160, LocalNode>,
    sorted_nodes: Vec<U160>,

    last_skip_pos: u32
}

/// Contains and manages a sorted list of connectable nodes and full information about them
impl NodeRepository {

    const max_map_score: u8 = 100;

    /// Based on the local score of nodes, get a list of the best ones to connect to
    /// This is primarily intended for startup, or when there are no nodes connected for whatever reason, and a connection is needed.
    pub fn get_nodes(&self) -> &Node {
        &self.available_nodes.get(&self.sorted_nodes[0]).unwrap().node
    }

    /// Notify the repository of updated or new node information. Will automatically add or change an existing node as appropriate based on the key in the repository
    pub fn apply(&mut self, node: Node) -> bool {
        {
            let mut an = &mut self.available_nodes;
            if an.contains_key(&node.key) {
                let score = an.get(&node.key).unwrap().score;

                // node already exists, so we must simply update in place
                let k = node.key;
                return an.insert(k, LocalNode {
                    node: node,
                    score: score
                }).unwrap().node == an.get(&k).unwrap().node; // check to see if it was changed
            }
        }

        let n = LocalNode::new(node);
        
        self.sorted_nodes.push(n.node.key);
        self.available_nodes.insert(n.node.key, n);

        true
    }

    /// Increment the connection score for the given node ID. Does nothing if the node does not exist in the repo, so call after apply() if the node is new.
    /// Returns whether or not a change was made to the repo
    pub fn upScore(&mut self, node: U160) -> bool {
        if let Some(n) = self.available_nodes.get_mut(&node) {
            n.score += 1;
        }
        else {
            return false;
        }

        self.resort();
        true
    }

    /// Decrement the connection score for the given node ID. Does nothing if the node does not exist in the repo
    /// Returns whether or not a change was made to the repo
    pub fn downScore(&mut self, node: U160) -> bool {
        if let Some(n) = self.available_nodes.get_mut(&node) {
            if n.score > 0 {
                n.score /= 2;
            }
            else {
                return false;
            }
        }
        else {
            return false;
        }

        self.resort();
        true
    }

    fn build(&mut self, nodes: &Vec<LocalNode>) {
        self.available_nodes = HashMap::new();
        self.sorted_nodes = Vec::new();

        for node in nodes {
            let n = LocalNode::clone(node);

            self.sorted_nodes.push(node.node.key);
            self.available_nodes.insert(node.node.key, n);
        }

        self.resort();
    }

    pub fn trim(&mut self) {
        let SAVED_NODES_COUNT = 100;
        let nodes: Vec<LocalNode> = self.available_nodes.values().take(SAVED_NODES_COUNT).cloned().collect();
        self.build(&nodes);
    }

    pub fn save(&self) -> Result<u32, Error> {

        let saved: Vec<&LocalNode> = self.available_nodes.values().collect();
        
        let serialized = serde_json::to_string_pretty(&saved).unwrap();

        // open a file, put serialized data into it
        match File::create(&self.node_store_path()) {
            Ok(mut f) => {
                write!(f, "{}", serialized);
                Ok(saved.len() as u32)
            },
            Err(e) => Err(e)
        }
    }

    pub fn load(&mut self) -> Result<u32, Error> {
        info!("Load stored nodes from file...");

        // Handle file does not exist
        if !self.node_store_path().as_path().is_file() {
            // import seed nodes, and return 0 to indicate that the file was not there
            return Ok(0);
        }

        match File::open(&self.node_store_path().as_path()) {
            Ok(mut f) => {
                let mut contents = String::new();
                f.read_to_string(&mut contents).expect("Unexpected node file read error");

                let loaded: Vec<LocalNode> = serde_json::from_str(&contents).unwrap();
                self.build(&loaded);

                Ok(self.available_nodes.len() as u32)
            },
            Err(e) => Err(e)
        }
    }

    fn node_store_path(&self) -> PathBuf {
        let mut p = get_storage_dir().unwrap();
        p.push("nodes.json");

        p
    }

    fn resort(&mut self) {

        let mut an = &self.available_nodes;
        self.sorted_nodes.sort_by(
            |a,b| an.get(b).unwrap().score.cmp(&an.get(a).unwrap().score)
        )
    }
}