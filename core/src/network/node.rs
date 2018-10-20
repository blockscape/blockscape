use serde_json;
use std::cmp::*;
use std::fmt;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write, Error, ErrorKind};
use std::error::Error as BaseError;
use std::ffi::CString;
use std::ptr;
use std::mem;
use std::net::{SocketAddr,IpAddr,Ipv4Addr,Ipv6Addr};
use std::path::*;
use std::str::FromStr;

use futures::prelude::*;
use futures::future;

use libc;

use env::get_storage_dir;
use hash::hash_pub_key;
use primitives::U160;
use worker::WORKER;

#[derive(Clone, Serialize, PartialEq, Eq, Deserialize, Hash)]
pub enum Protocol {
    Tcp,
    Udp
}

/// All the information needed to make contact with a remote node
#[derive(Clone, Serialize, PartialEq, Eq, Deserialize, Hash)]
pub struct NodeEndpoint {

    pub protocol: Protocol,

    /// Network IP of the client
    pub host: String,

    /// Port for UDP communication of node
    pub port: u16,
}

impl NodeEndpoint {
    pub fn new_from_sockaddr(protocol: Protocol, sa: SocketAddr) -> NodeEndpoint {
        let port = sa.port();

        NodeEndpoint {
            protocol,
            host: format!("{}", sa.ip()),
            port
        }
    }

    pub fn as_socketaddr(self) -> Box<Future<Item=SocketAddr, Error=Error>> {
		
		if let Ok(ip) = self.host.parse::<IpAddr>() {
			return Box::new(future::ok(SocketAddr::new(ip, self.port)))
		}
		
		let c_host = CString::new(self.host.clone());
		if c_host.is_err() {
			// there was a null byte in the hostname
			return Box::new(future::err(Error::new(ErrorKind::AddrNotAvailable, "Hostname includes invalid character")));
		}
		
		// must be spawned on a worker thread because getaddrinfo() is a blocking call
		Box::new(WORKER.spawn_fn(move || {
			let addr: Result<IpAddr, Error> = unsafe {
				let mut res = ptr::null_mut();
				let hn = c_host.unwrap();
				if libc::getaddrinfo(hn.as_ptr(), ptr::null(), &mem::zeroed(), &mut res) == 0 {
					// we have at least one address; for now take only a single address
					let ai: libc::addrinfo = *res;
					let sa_family: libc::sa_family_t = *(ai.ai_addr as *const u16);
					
					let ret = match sa_family as i32 {
						// remember, we have to swap bytes to big endian
						libc::AF_INET => Ok(IpAddr::V4(Ipv4Addr::from((*(ai.ai_addr as *const libc::sockaddr_in)).sin_addr.s_addr.to_be()))),
						libc::AF_INET6 => Ok(IpAddr::V6(Ipv6Addr::from((*(ai.ai_addr as *const libc::sockaddr_in6)).sin6_addr.s6_addr))),
						// unrecognized protocol?
						_ => Err(Error::new(ErrorKind::Other, "Unrecognized protocol"))
					};
					
					libc::freeaddrinfo(res);
					
					ret
				}
				else {
					Err(Error::new(ErrorKind::AddrNotAvailable, format!("No IPs found for hostname: {:?}", hn)))
				}
			};
			
			Box::new(future::result(addr))
		}).map(move |p| SocketAddr::new(p, self.port)))
    }
}

impl FromStr for NodeEndpoint {

    type Err = String;
    
    /// Convert from (tcp:|udp:)?<hostname>:<port> format to a node endpoint
    /// # Errors
    /// * If the format is not correct
    /// * If the port is not a parsable u16
    /// # Note
    /// * This does not check hostname validity, or perform any async blocking operation.
    fn from_str(v: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = v.split(':').collect();

        if parts.len() == 2 {
            Ok(NodeEndpoint {
                host: String::from(parts[0]),
                port: parts[1].parse::<u16>().map_err(|e| String::from(e.description()))?,
                protocol: Protocol::Udp // assume the UDP protocol by default
            })
        }
        else if parts.len() == 3 {
            Ok(NodeEndpoint {
                host: String::from(parts[1]),
                port: parts[2].parse::<u16>().map_err(|e| String::from(e.description()))?,
                protocol: match parts[0] {
                    "tcp" => Ok(Protocol::Tcp),
                    "udp" => Ok(Protocol::Udp),
                    _ => Err("Invalid packet protocol")
                }?
            })
        }
        else {
            Err(String::from("Invalid hostname string"))
        }
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
    available_nodes: HashMap<U160, LocalNode>,
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
    pub fn get_nodes(&self, idx: usize) -> &Node {
        &self.available_nodes.get(&self.sorted_nodes[idx % self.sorted_nodes.len()]).expect("node repository should not be empty on query").node
    }

    pub fn get(&self, node: &U160) -> Option<&Node> {
        self.available_nodes.get(node).map(|n| &n.node)
    }

    /// Notify the repository of updated or new node information. Will automatically add or change an existing node as appropriate based on the key in the repository
    pub fn apply(&mut self, node: Node) -> bool {
        let hpk = node.get_hash_id();
        {
            if let Some(n) = self.available_nodes.get_mut(&hpk) {

                if n.node != node {
                    n.node = node;
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
        self.available_nodes.insert(hpk, n);

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
        if let Some(n) = self.available_nodes.get_mut(&node) {
            n.score = n.score + 1;
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
            let n = LocalNode::clone(node);

            self.sorted_nodes.push(hpk);
            self.available_nodes.insert(hpk, n);
        }

        self.resort();
        self.changes += 1;
    }

    pub fn trim(&mut self) {
        debug!("Trimming saved nodes to {}", NodeRepository::SAVED_NODES_COUNT);
        let nodes: Vec<LocalNode> = self.available_nodes.values().take(NodeRepository::SAVED_NODES_COUNT).map(|n| n.clone()).collect();
        self.build(&nodes);
    }

    pub fn save(&mut self, name: &str) -> Result<u32, Error> {

        if self.changes == 0 {
            // suppress saving, since it is not necessary to do it
            return Ok(self.sorted_nodes.len() as u32);
        }

        let saved: Vec<LocalNode> = self.available_nodes.values().map(|n| n.clone()).collect();
        
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
            |a,b| an.get(b).unwrap().score.cmp(&an.get(a).unwrap().score)
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
					protocol: Protocol::Udp,
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
					protocol: Protocol::Udp,
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
					protocol: Protocol::Udp,
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
			protocol: Protocol::Udp,
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
