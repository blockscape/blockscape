use std::cell::{RefCell,Cell};
use std::io;
use std::rc::Rc;
use std::net::SocketAddr;

use futures::prelude::*;
use futures::sink::BoxSink;
use futures::stream;

use primitives::U256;
use time::Time;
use signer::RSA_KEY_SIZE;

use network::context::NetworkContext;
use network::node::Node;
use network::protocol::*;
use network::job::*;
use network::shard::ShardInfo;

pub trait Session {

    /// Called when a ping packet has been returned with a pong, with the given timestamp
    fn recv_ping(&self, time: Time);

    /// Verify that everything is still active with the connection. Send a ping packet to verify the other end can still communicate with us.
    fn check_conn(&self);

    /// Poll call for work to be done on the job. If the job does not appear to be making any progress and times out, it is returned here
    fn check_job(&self) -> Option<NetworkJob>;

    /// Attempts to assign the given job to this node and send requests to resolve it. Will return whether or not the node
    /// was able to accept the job (depending on what it's current job happened to be at the time)
    fn assign_job(&self, job: &NetworkJob) -> bool;

    /// Attempt to change, or augment, and existing job given the equivalent job. This will return true if any job was modified.
    fn update_job(&self, job: &NetworkJob) -> bool;

    /// Indicate that the user has given a sign of ill-intent or is misusing the connection. This will automatically disconnect from the client if the abuse limit has been exceeded.
    fn mark_abuse(&self);

    /// Diagnostic and statistical information about this peer connection
    fn get_info(&self) -> SessionInfo;

    /// Appends a bye packet to the end of the queue
    /// NOTE: Dont forget to empty the send queue after calling this function!
    fn close(&self);

    /// Returns if this connection has received a "bye" packet and should be slated for closure
    fn is_done(&self) -> Option<ByeReason>;

    /// Returns if this connection has been confirmed on both sides and is "alive"
    fn is_introduced(&self) -> bool;

    /// Get the network_id served by this client connection
    fn get_network_id(&self) -> &U256;

    /// Get the network context
    fn get_context(&self) -> &Rc<NetworkContext>;
}

/// Statistical information object for detailed information about a session
#[derive(Serialize, Deserialize, Debug)]
pub struct SessionInfo {
    pub network_id: U256,
    pub peer: Node,
    pub latency: Time,
    pub established_since: Time
}

pub struct NewSessionOptions {
    pub context: Rc<NetworkContext>, 
    pub local_port: u8, 
    pub remote_peer: Node,
    pub remote_addr: SocketAddr,
    pub network_id: U256, 
    pub sink: Option<BoxSink<Packet, io::Error>>
}


pub struct GenericSession {

    context: Rc<NetworkContext>,

    /// Indicates if the session has completed
    done: Cell<Option<ByeReason>>,

    /// The shard of interest for this session
    network_id: U256,

    /// Information about the node on the other end. If this is unset, then the connection is not really fully initialized yet
    remote_peer: Node,
 
    /// The port which we connect to on the other end. This starts as 255 for new connections
    remote_port: u8,

    /// Helper variable for router to manage multiple connections from a single client
    local_port: u8,

    /// Latest address information on the remote client (different from NodeEndpoint)
    remote_addr: SocketAddr,

    sink: Cell<Option<BoxSink<Packet, io::Error>>>,

    /// When we first were initialized
    established_since: Time,

    /// Average latency over the last n ping-pong sequences, round trip
    latency: Cell<Time>,

    /// Time at which the most recent ping packet was sent
    last_ping_send: Cell<Option<Time>>,

    /// Assigned jobs
    current_job: RefCell<Option<(NetworkJob, u32, Time)>>,

    /// The unique packet identifier to use for the next packet
    current_seq: Cell<u32>,

    /// Used to help discern the number of failed replies. When this number exceeds a threshold,
    /// the connection is considered dropped
    strikes: Cell<u32>,

    /// Used to track the number of times the node has misbehaved/sent bogus data over the last time period. Too many abuses leads to blacklisting.
    abuses: Cell<u32>
}

impl GenericSession {
    pub fn new(opts: NewSessionOptions) -> GenericSession {

		GenericSession {
            context: opts.context,
            local_port: opts.local_port,
            done: Cell::new(None),
            remote_peer: opts.remote_peer,
            remote_addr: opts.remote_addr,
            sink: Cell::new(opts.sink),
            remote_port: 255,
            network_id: opts.network_id,
            established_since: Time::current(),
            latency:  Cell::new(Time::from_milliseconds(0)),
            last_ping_send: Cell::new(None),
            current_seq: Cell::new(0),
            current_job: RefCell::new(None),
            strikes: Cell::new(0),
            abuses: Cell::new(0)
        }
    }

    fn send_packet(&self, pl: Packet) {
        let sink = self.sink.replace(None);
        if sink.is_some() {
            // TODO: Can this be made more efficient?
            let st = stream::iter_ok::<_, io::Error>(vec![pl]);
            // TODO: Try to eliminate call to wait! Typically it should not be an issue, but
            // it would be more future-ist to provide some way to react upon future availability
            match st.forward(sink.unwrap()).wait() {
				Ok((_, sock)) => self.sink.set(Some(sock)),
				Err(err) => {
					warn!("Connection to peer broken: {:?}", err);
					self.close();
				}
            }
        }
        else {
            let sp = SocketPacket(self.remote_addr.clone(), RawPacket {
                port: self.remote_port,
                payload: pl
            });

            self.context.udp_send_packets(vec![sp]);
        }
    }
    
    pub fn send(&self, msg: Message, signed: bool) -> u32 {
        let seq = self.current_seq.replace(self.current_seq.get() + 1);

        let mut pl = Packet::new(seq, msg);

        if signed {
            pl = pl.apply_sig(&self.context.config.private_key);
        }

        self.send_packet(pl);

        seq
    }

    pub fn send_reply(&self, msg: Message, seq: u32, signed: bool) {
        let mut pl = Packet::new(seq, msg);

        if signed {
            pl = pl.apply_sig(&self.context.config.private_key);
        }

        self.send_packet(pl)
    }

    pub fn handle_introduce(mut self, p: &Packet, new_addr: &SocketAddr) -> GenericSession {

        if let &Message::Introduce { ref node, ref port, .. } = &p.msg {

            if *port == 255 {
                // invalid port to receive for introduce
                warn!("Introduce packet has invalid port value. Ignoring...");
                return self;
            }

            let was_introduced = self.is_introduced();

            if self.is_introduced() {

                // prevent connection hijacking
                if !p.check_sig(&self.remote_peer) {
                    // drop packet
                    debug!("Unsigned introduce upgrade packet received; ignoring");
                    return self;
                }

                self.remote_addr = *new_addr;
            }
            
            self.remote_peer = node.clone();
            self.remote_port = *port;
            self.strikes.set(0);

            // nodes must all share the same key length
            if node.key.len() != self.context.my_node.key.len() {
                debug!("Key size is wrong from client: {:?}, expected: {:?}, actual: {:?}", node.endpoint, node.key.len(), RSA_KEY_SIZE);
                self.done.set(Some(ByeReason::ExitPermanent));
            }

            // detect if we have connected to self
            if node.key == self.context.my_node.key {
                debug!("Detected a connection to self, from remote: {:?}", node.endpoint);
                self.done.set(Some(ByeReason::ExitPermanent));
            }

            if !was_introduced {
                // send back a reply
                self.send_introduce();
            }
        }
        else {
            panic!("Received non-introduce packet for session init");
        }

        self
    }

    #[inline]
    pub fn send_introduce(&self) {
        self.send(Message::Introduce {
                node: self.context.my_node.clone(),
                port: self.local_port,
                network_id: self.network_id
            }, true);
    }

    /// Provide a packet which has been received for this session
    /// Returns a networkjob which would be finished by this packet
    pub fn recv(myself: &Rc<GenericSession>, packet: &Packet, shard: &ShardInfo) -> Option<NetworkJob> {

        if myself.done.get().is_some() {
            return None; // no need to do any additional processing
        }

        if !myself.is_introduced() {
            // we cannot take this packet
            match packet.msg {
                Message::Introduce { .. } => {
                    unreachable!("Tried to recv Introduce packet (not captured)");
                },

                _ => {
                    // must receive introduce packet first
                    myself.done.set(Some(ByeReason::Exit));
                    return None;
                }
            }
        }
        else {
            packet.handle(myself, shard);

            if let Some((job, seq, time)) = myself.current_job.replace(None) {
                if seq == packet.seq {
                    return Some(job);
                }
                else {
                    // put it back
                    myself.current_job.replace(Some((job, seq, time)));
                }
            }
        }

        None
    }

    #[inline]
    pub fn get_remote_node(&self) -> &Node {
        // pulling an arc out of a cell basically requires two swaps
        &self.remote_peer
    }

    #[inline]
    pub fn get_remote_addr(&self) -> &SocketAddr {
        &self.remote_addr
    }
}

impl Session for GenericSession {
    fn recv_ping(&self, time: Time) {
        if let Some(lps) = self.last_ping_send.get() {
            if lps == time {
                let f = 1.0 / PING_RETENTION;
                let mut l = self.latency.get();
                l.apply_weight(&lps.diff(&time), f);
                self.latency.set(l);
            }

            self.last_ping_send.set(None);
            // now we know the connection is still going, reset strike counter
            self.strikes.set(0);
        }
    }

    /// Performs checks to verify the current connection state. If the connection appears dead, it will
    /// set this connection as done. Otherwise, it will send a ping packet.
    /// Call this function at regular intervals for best results.
    fn check_conn(&self) {
        if self.done.get().is_none() {

            if !self.is_introduced() {
                // we might have to re-send the introduce packet
                let introduce_n = self.context.my_node.clone();

                self.send(Message::Introduce {
                        node: introduce_n,
                        port: self.local_port,
                        network_id: self.network_id
                }, false);

                if self.strikes.replace(self.strikes.get() + 1) + 1 > TIMEOUT_TOLERANCE as u32 {
                    self.done.set(Some(ByeReason::Timeout));
                }
            }
            else {
                // if we still have an outgoing ping and too much time has passed, add a strike
                if let Some(lps) = self.last_ping_send.get() {
                    if lps.diff(&Time::current()).millis() > PING_TIMEOUT {
                        self.strikes.set(self.strikes.get() + 1);
                    }
                }

                //debug!("Connection Strikes: {}", self.strikes.load(Relaxed));

                if self.strikes.get() > TIMEOUT_TOLERANCE as u32 {
                    self.done.set(Some(ByeReason::Timeout));
                }
                else {

                    let lps = Time::current();

                    self.send(Message::Ping(lps), false);
                    self.last_ping_send.set(Some(lps));
                }
            }
        }
    }

    fn check_job(&self) -> Option<NetworkJob> {

        let j = self.current_job.replace(None);

        if let Some((job, seq, time)) = j {
            // has it expired?
            if time.diff(&Time::current_local()) > Time::from_milliseconds(JOB_TIMEOUT) {
                Some(job)
            }
            else {
                // put it back, since we are not done with it yet
                self.current_job.replace(Some((job, seq, time)));
                None
            }
        }
        else {
            None
        }
    }

    fn assign_job(&self, job: &NetworkJob) -> bool {

        debug!("Assign Job: {:?}", job);

        if !self.is_introduced() {
            return false; // cannot do job if we are not fully initialized
        }

        // make and send the packet involved with the job
        let seq = self.send(job.make_req(&self.context), false);

        let mut j = self.current_job.borrow_mut();
        if j.is_some() {
            return false;
        }

        *j = Some((job.clone(), seq, Time::current_local()));

        true
    }

    fn update_job(&self, job: &NetworkJob) -> bool {
        // try to augment our current job
        if let Some((ref mut j, _, _)) = self.current_job.borrow_mut().as_mut() {
            // returns if the job was modified
            j.augment(job)
        }
        else {
            false
        }
    }

    fn mark_abuse(&self) {
        self.abuses.set(self.abuses.get() + 1);

        // disconnect automatically if we have exceeded the abuse count
        if self.abuses.get() > MAX_ABUSES as u32 {
            self.send(Message::Bye(ByeReason::Abuse), false);
            self.done.set(Some(ByeReason::Abuse));
        }
    }

    fn get_info(&self) -> SessionInfo {
        SessionInfo {
            peer: self.remote_peer.clone(),
            network_id: self.network_id,
            latency: self.latency.get(),
            established_since: self.established_since
        }
    }

    fn get_network_id(&self) -> &U256 {
        &self.network_id
    }

    fn get_context(&self) -> &Rc<NetworkContext> {
        &self.context
    }

    fn close(&self) {
        self.send(Message::Bye(ByeReason::Exit), false);
        self.done.set(Some(ByeReason::Exit));
    }

    fn is_done(&self) -> Option<ByeReason> {
        self.done.get()
    }

    fn is_introduced(&self) -> bool {
        self.remote_port != 255
    }
}
