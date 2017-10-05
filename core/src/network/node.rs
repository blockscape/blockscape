use std::net::{SocketAddr,IpAddr};
use super::U160;
use std::sync::Arc;

use std::collections::HashMap;
use std::fs::File;
use super::env::get_storage_dir;
use dns_lookup::lookup_host;
use serde_json;

use openssl::pkey::PKey;

use std::path::*;
use std::cmp::*;

use std::io::{Read, Write, Error};

use hash::hash_pub_key;

use rand;

/// All the information needed to make contact with a remote node
#[derive(Clone, Serialize, PartialEq, Eq, Deserialize, Debug)]
pub struct NodeEndpoint {
    /// Network IP of the client
    pub host: String,

    /// Port for UDP communication of node
    pub port: u16,
}

impl NodeEndpoint {
    pub fn as_socketaddr(self) -> Option<SocketAddr> {
        // DNS resolve if necessary
        let mut ip = match self.host.parse::<IpAddr>() {
            Ok(ip) => Some(ip),
            Err(err) => lookup_host(self.host.as_str())
                .ok()
                .map(|r| r.first().cloned())
                .unwrap_or(None)
        };

        ip.map(|p| SocketAddr::new(p, self.port))
    }
}

/// Detailed information about a node
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
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
    available_nodes: HashMap<U160, LocalNode>,
    sorted_nodes: Vec<U160>
}

/// Contains and manages a sorted list of connectable nodes and full information about them
impl NodeRepository {

    const max_map_score: u8 = 100;

    pub fn new() -> NodeRepository {
        let mut nr = NodeRepository {
            available_nodes: HashMap::new(),
            sorted_nodes: Vec::new()
        };

        nr.build(&vec![]); // initialize empty, which will cause the seed nodes to be populated
        nr
    }

    /// Based on the local score of nodes, get a list of the best ones to connect to
    /// This is primarily intended for startup, or when there are no nodes connected for whatever reason, and a connection is needed.
    pub fn get_nodes(&self, idx: usize) -> Arc<Node> {
        Arc::new(self.available_nodes.get(&self.sorted_nodes[idx % self.sorted_nodes.len()]).cloned().unwrap().node)
    }

    pub fn get(&self, node: &U160) -> Option<Arc<Node>> {
        self.available_nodes.get(node).map(|n| Arc::new(n.node.clone()))
    }

    /// Notify the repository of updated or new node information. Will automatically add or change an existing node as appropriate based on the key in the repository
    pub fn apply(&mut self, node: Node) -> bool {
        let hpk = hash_pub_key(&node.key[..]);
        {
            let mut an = &mut self.available_nodes;
            if an.contains_key(&hpk) {
                let score = an.get(&hpk).unwrap().score;

                // node already exists, so we must simply update in place
                return an.insert(hpk, LocalNode {
                    node: node,
                    score: score
                }).unwrap().node == an.get(&hpk).unwrap().node; // check to see if it was changed
            }
        }

        let n = LocalNode::new(node);
        
        self.sorted_nodes.push(hpk);
        self.available_nodes.insert(hpk, n);

        true
    }

    /// Remove the given node from the repository. This should only be done if the node data is
    /// found to be bogus
    pub fn remove(&mut self, node: &U160) {
        self.available_nodes.remove(node);
        self.sorted_nodes.retain(|n| n != node);
    }

    /// Increment the connection score for the given node ID. Does nothing if the node does not exist in the repo, so call after apply() if the node is new.
    /// Returns whether or not a change was made to the repo
    pub fn upScore(&mut self, node: &U160) -> bool {
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
    pub fn downScore(&mut self, node: &U160) -> bool {
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

    pub fn build(&mut self, nodes: &Vec<LocalNode>) {
        self.available_nodes = HashMap::new();
        self.sorted_nodes = Vec::new();

        let seed_node_vec = vec![LocalNode {
            node: Node {
                endpoint: NodeEndpoint {
                    host: String::from("seed-1.blockscape"),
                    port: 42224
                },
                key: vec![1],
                version: 1,
                name: String::from("Seed Node 1")
            },
            score: 10
        },
        LocalNode {
            node: Node {
                endpoint: NodeEndpoint {
                    host: String::from("seed-2.blockscape"),
                    port: 42224
                },
                key: vec![2],
                version: 1,
                name: String::from("Seed Node 2")
            },
            score: 10
        }];

        let imported = match nodes.len() {
            // I would put the below stuff into a constant, but making method calls (however constructive) is not allowed, so I must put it here.
            0 => &seed_node_vec,
            _ => nodes
        };

        for node in imported {
            let n = LocalNode::clone(node);
            let hpk = hash_pub_key(&n.node.key[..]);

            self.sorted_nodes.push(hpk);
            self.available_nodes.insert(hpk, n);
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

#[test]
fn populated_seed_nodes() {
    let nr = NodeRepository::new();

    assert_eq!(nr.get_nodes(0).name, "Seed Node 1");
    assert_eq!(nr.get_nodes(1).name, "Seed Node 2");
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
                key: U160::from(1),
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
                key: U160::from(2),
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
                key: U160::from(3),
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

    // should still work if we add another node and score it up a bit
    nr.apply(Node {
        endpoint: NodeEndpoint {
            host: String::from("supertest-4.blockscape"),
            port: 42224
        },
        key: U160::from(4),
        version: 1,
        name: String::from("SuperTest Node 4")
    });
    nr.upScore(U160::from(4));
    nr.upScore(U160::from(4));
    nr.upScore(U160::from(4));

    assert_eq!(nr.get_nodes(0).name, "SuperTest Node 2");
    assert_eq!(nr.get_nodes(1).name, "SuperTest Node 4");
    assert_eq!(nr.get_nodes(2).name, "SuperTest Node 3");
    assert_eq!(nr.get_nodes(3).name, "SuperTest Node 1");
}