use std::collections::HashMap;
use std::net::UdpSocket;
use std::io::Error;

use super::env::get_client_name;

use u256::*;
use u160::*;

use network::session::Session;
use network::node::*;

pub struct DataStore;

pub struct Client<'a> {
    /// Independant "connections" to each of the other NodeEndpoints we interact with
    sessions: HashMap<U160, Session>,

    /// The node object which represents my own system
    my_node: Node,

    node_repo: &'a NodeRepository,

    db: &'a DataStore,

    /// List of all the networks we should be seeking node connections with
    connected_networks: Vec<U256>,
    /// The socket used to accept and invoke UDP communication
    socket: Option<UdpSocket>,
}

impl<'a> Client<'a> {

    pub fn new(db: &'a DataStore, node_repo: &'a NodeRepository) -> Client<'a> {
        
        Client {
            db: db,
            node_repo: node_repo,
            connected_networks: Vec::new(),
            sessions: HashMap::new(),
            socket: None,
            my_node: Node {
                key: U160::from(100),// TODO: This is stupid
                version: Session::PROTOCOL_VERSION,
                endpoint: NodeEndpoint { host: String::from(""), port: 0 },
                name: get_client_name()
            }
        }

        // Build my node object
    }

    pub fn open(&mut self, addr: String, port: u16) -> Result<(), Error> {
        let addr_port = format!("{}:{}", addr, port);
        match UdpSocket::bind(addr_port) {
            Ok(s) => {
                self.socket = Some(s);

                // create a node endpoint and apply it to my node
                self.my_node.endpoint = NodeEndpoint {
                    host: addr,
                    port: port
                };

                // Form connections to some known nodes
                let nodes = self.node_repo.get_nodes(0);

                Ok(())
            },
            Err(e) => Err(e)
        }
    }

    pub fn run(self) {

    }
}