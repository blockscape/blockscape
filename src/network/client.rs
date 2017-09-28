use std::collections::HashMap;
use std::net::UdpSocket;

pub struct Client {
    /// Independant "connections" to each of the other NodeEndpoints we interact with
    sessions: HashMap<U160, Session>,
    /// List of all the networks we should be seeking node connections with
    connected_networks: Vec<U256>,
    /// The socket used to accept and invoke UDP communication
    socket: UdpSocket,
}

impl Client {
    fn open(addr: str, port: u16) {
        socket = UdpSocket::bind(addr + ":" + port);

        // Form connections to some known nodes
    }

    fn run() {

    }

    fn close() {

    }
}