use dns_lookup::lookup_host;
use serde_json;
use std::cmp::*;
use std::fmt;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write, Error};
use std::error::Error as BaseError;
use std::net::{SocketAddr,IpAddr};
use std::path::*;
use std::sync::Arc;
use parking_lot::RwLock;
use std::str::FromStr;

use env::get_storage_dir;
use hash::hash_pub_key;
use primitives::U160;

/// All the information needed to make contact with a remote node
#[derive(Clone, Serialize, PartialEq, Eq, Deserialize, Hash)]
pub struct NodeEndpoint {
    /// Network IP of the client
    pub host: String,

    /// Port for UDP communication of node
    pub port: u16,
}

impl NodeEndpoint {
    pub fn as_socketaddr(self) -> Option<SocketAddr> {
        // DNS resolve if necessary
        let ip = match self.host.parse::<IpAddr>() {
            Ok(ip) => Some(ip),
            Err(_) => lookup_host(self.host.as_str())
                .ok()
                .map(|r| r.first().cloned())
                .unwrap_or(None)
        };

        ip.map(|p| SocketAddr::new(p, self.port))
    }
}

impl FromStr for NodeEndpoint {

    type Err = String;
    
    /// Convert from <hostname>:<port> format to a node endpoint
    /// # Errors
    /// * If the format is not correct
    /// * If the port is not a parsable u16
    /// # Note
    /// * This does not check hostname validity, or perform any async blocking operation.
    fn from_str(v: &str) -> Result<Self, Self::Err> {
        let mut parts = v.split(':');

        let host = parts.next().ok_or(String::from("Missing hostname part"))?;
        let port = parts.next().ok_or(String::from("Missing port part"))?.parse::<u16>().map_err(|e| String::from(e.description()))?;

        Ok(NodeEndpoint {
            host: String::from(host),
            port: port
        })
    }
}

impl fmt::Display for NodeEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

impl fmt::Debug for NodeEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// Detailed information about a node
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct Node {
    /// Information on the address and port which can be used to open a connection to this node
    pub endpoint: NodeEndpoint,
    /// Public key hash derived used for signing messages
    pub key: Vec<u8>,
    /// The version of the node's network communications. Incremented on any fundamental network change between releases
    pub version: u16,
    /// A description for the client, consisting typically of the name of the client, plus the version code
    pub name: String
}

impl Node {
    /// Minimalist constructor for if you only have the endpoint (which is the minimum required)
    pub fn new(endpoint: NodeEndpoint) -> Node {
        Node {
            endpoint: endpoint,
            key: Vec::new(),
            version: 1,
            name: String::new()
        }
    }

    pub fn get_hash_id(&self) -> U160 {
        hash_pub_key(&self.key[..])
    }
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
pub struct LocalNode {
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

#[derive(Debug)]
pub struct NodeRepository {
    available_nodes: HashMap<U160, RwLock<LocalNode>>,
    sorted_nodes: Vec<U160>,
    changes: usize
}

/// Contains and manages a sorted list of connectable nodes and full information about them
impl NodeRepository {

    const SAVED_NODES_COUNT: usize = 100;

    pub fn new() -> NodeRepository {
        let mut nr = NodeRepository {
            available_nodes: HashMap::new(),
            sorted_nodes: Vec::new(),
            changes: 0
        };

        nr.build(&vec![]); // initialize empty, which will cause the seed nodes to be populated
        nr
    }

    /// Based on the local score of nodes, get a list of the best ones to connect to
    /// This is primarily intended for startup, or when there are no nodes connected for whatever reason, and a connection is needed.
    /// # Panics
    /// * If the repository is empty (i.e. has 0 nodes to connect to). You should check this on your end.
    pub fn get_nodes(&self, idx: usize) -> Arc<Node> {
        Arc::new(self.available_nodes.get(&self.sorted_nodes[idx % self.sorted_nodes.len()]).map(|n| n.read().clone()).unwrap().node)
    }

    pub fn get(&self, node: &U160) -> Option<Arc<Node>> {
        self.available_nodes.get(node).map(|n| Arc::new(n.read().node.clone()))
    }

    /// Notify the repository of updated or new node information. Will automatically add or change an existing node as appropriate based on the key in the repository
    pub fn apply(&mut self, node: Node) -> bool {
        let hpk = node.get_hash_id();
        {
            if let Some(n) = self.available_nodes.get(&hpk) {

                let mut myn = n.write();
                if myn.node != node {
                    myn.node = node;
                    self.changes += 1;
                    return true;
                }
            }
        }

        false
    }

    pub fn new_node(&mut self, node: Node) {

        let hpk = node.get_hash_id();
        // sanity check
        if self.available_nodes.contains_key(&hpk) {
            self.apply(node);
            return;
        }
        let n = LocalNode::new(node);
        
        self.sorted_nodes.push(hpk);
        self.available_nodes.insert(hpk, RwLock::new(n));

        self.changes += 1;
    }

    /// Remove the given node from the repository. This should only be done if the node data is
    /// found to be bogus
    pub fn remove(&mut self, node: &U160) {

        if self.available_nodes.contains_key(node) {
            self.available_nodes.remove(node);
            self.sorted_nodes.retain(|n| n != node);

            self.changes += 1;
        }
    }

    /// Increment the connection score for the given node ID. Does nothing if the node does not exist in the repo, so call after apply() if the node is new.
    /// Returns whether or not a change was made to the repo
    pub fn up_score(&mut self, node: &U160) -> bool {
        if let Some(n) = self.available_nodes.get(&node) {
            let mut n2 = n.write();
            n2.score = n2.score + 1;
        }
        else {
            return false;
        }

        self.resort();

        self.changes += 1;
        true
    }

    /// Decrement the connection score for the given node ID. Does nothing if the node does not exist in the repo
    /// Returns whether or not a change was made to the repo
    pub fn down_score(&mut self, node: &U160) -> bool {
        if let Some(n) = self.available_nodes.get(&node) {
            let mut n2 = n.write();
            if n2.score > 0 {
                n2.score /= 2;
            }
            else {
                return false;
            }
        }
        else {
            return false;
        }

        self.resort();

        self.changes += 1;
        true
    }

    pub fn len(&self) -> usize {
        self.sorted_nodes.len()
    }

    pub fn build(&mut self, nodes: &Vec<LocalNode>) {
        self.available_nodes = HashMap::new();
        self.sorted_nodes = Vec::new();

        let imported = match nodes.len() {
            // I would put the below stuff into a constant, but making method calls (however constructive) is not allowed, so I must put it here.
            0 => return,
            _ => nodes
        };

        for node in imported {
            let hpk = node.node.get_hash_id();
            let n = RwLock::new(LocalNode::clone(node));

            self.sorted_nodes.push(hpk);
            self.available_nodes.insert(hpk, n);
        }

        self.resort();
        self.changes += 1;
    }

    pub fn trim(&mut self) {
        debug!("Trimming saved nodes to {}", NodeRepository::SAVED_NODES_COUNT);
        let nodes: Vec<LocalNode> = self.available_nodes.values().take(NodeRepository::SAVED_NODES_COUNT).map(|n| n.read().clone()).collect();
        self.build(&nodes);
    }

    pub fn save(&mut self, name: &str) -> Result<u32, Error> {

        if self.changes == 0 {
            // suppress saving, since it is not necessary to do it
            return Ok(self.sorted_nodes.len() as u32);
        }

        let saved: Vec<LocalNode> = self.available_nodes.values().map(|n| n.read().clone()).collect();
        
        let serialized = serde_json::to_string_pretty(&saved).unwrap();

        debug!("Save file to: {:?}", self.node_store_path(name).as_path());

        // open a file, put serialized data into it
        match File::create(&self.node_store_path(name)) {
            Ok(mut f) => {
                write!(f, "{}", serialized).expect("Could not save to open file");
                self.changes = 0;

                Ok(saved.len() as u32)
            },
            Err(e) => Err(e)
        }
    }

    pub fn load(&mut self, name: &str) -> Result<u32, Error> {
        info!("Load stored nodes from file...");

        // Handle file does not exist
        if !self.node_store_path(name).as_path().is_file() {
            // import seed nodes, and return 0 to indicate that the file was not there
            return Ok(0);
        }

        match File::open(&self.node_store_path(name).as_path()) {
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

    fn node_store_path(&self, name: &str) -> PathBuf {
        let mut p = get_storage_dir().unwrap();
        p.push("nodes");
        p.push(name.to_owned() + ".json");

        p
    }

    fn resort(&mut self) {

        let an = &self.available_nodes;
        self.sorted_nodes.sort_by(
            |a,b| an.get(b).unwrap().read().score.cmp(&an.get(a).unwrap().read().score)
        )
    }
}

#[test]
fn populated_seed_nodes() {
    let nr = NodeRepository::new();

    assert_eq!(nr.len(), 0);

    //assert_eq!(nr.get_nodes(0).name, "Seed Node 1");
    //assert_eq!(nr.get_nodes(1).name, "Seed Node 2");
}

#[test]
fn custom_node_vec() {
    let mut nr = NodeRepository::new();

    nr.build(&vec![
        LocalNode {
            node: Node {
                endpoint: NodeEndpoint {
                    host: String::from("supertest-1.blockscape"),
                    port: 42224
                },
                key: vec![1],
                version: 1,
                name: String::from("SuperTest Node 1")
            },
            score: 1
        },
        LocalNode {
            node: Node {
                endpoint: NodeEndpoint {
                    host: String::from("supertest-2.blockscape"),
                    port: 42224
                },
                key: vec![2],
                version: 1,
                name: String::from("SuperTest Node 2")
            },
            score: 4
        },
        LocalNode {
            node: Node {
                endpoint: NodeEndpoint {
                    host: String::from("supertest-3.blockscape"),
                    port: 42224
                },
                key: vec![3],
                version: 1,
                name: String::from("SuperTest Node 3")
            },
            score: 2
        }
    ]);

    // first one should be the supertest 2 because it has a higher score
    assert_eq!(nr.get_nodes(0).name, "SuperTest Node 2");
    assert_eq!(nr.get_nodes(1).name, "SuperTest Node 3");
    assert_eq!(nr.get_nodes(2).name, "SuperTest Node 1");
    assert_eq!(nr.len(), 3);

    // should still work if we add another node and score it up a bit
    nr.new_node(Node {
        endpoint: NodeEndpoint {
            host: String::from("supertest-4.blockscape"),
            port: 42224
        },
        key: vec![4],
        version: 1,
        name: String::from("SuperTest Node 4")
    });

    let mykey = hash_pub_key(&[4]);

    nr.up_score(&mykey);
    nr.up_score(&mykey);
    nr.up_score(&mykey);

    assert_eq!(nr.get_nodes(0).name, "SuperTest Node 2");
    assert_eq!(nr.get_nodes(1).name, "SuperTest Node 4");
    assert_eq!(nr.get_nodes(2).name, "SuperTest Node 3");
    assert_eq!(nr.get_nodes(3).name, "SuperTest Node 1");
    assert_eq!(nr.len(), 4);
}