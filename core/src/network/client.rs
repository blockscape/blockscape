use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::rc::*;
use std::borrow::Borrow;
use std::thread;
use std::time::Duration;

use futures::prelude::*;
use futures::sink::BoxSink;
use futures::sync::*;
use futures::sync::mpsc::{UnboundedSender, unbounded};
use futures::future;

use openssl::pkey::PKey;

use tokio_core::reactor::*;
use tokio_core::net::{TcpListener, UdpSocket};
// needed for "framed"
use tokio_io::AsyncRead;

use primitives::U256;
use record_keeper::{RecordKeeper, RecordEvent};
use signer::generate_private_key;
use util::QuitSignal;

use network::context::*;
use network::node::{Node, NodeEndpoint, Protocol};
use network::job::NetworkJob;
//use network::ntp;
use network::protocol::*;
use network::session::SessionInfo;
use network::shard::ShardMode;
use network::tcp::TCPCodec;
use network::udp::UDPCodec;

pub trait BroadcastReceiver {
    /// Returns a unique identifier to separate events for this broadcast ID. Must be unique per application.
    fn get_broadcast_id(&self) -> u8;

    /// Called when a broadcast is received. If the broadcast is to be propogated, the broadcast event must be re-called.
    /// Internally, network automatically handles duplicate events as a result of the reliable flood, so that can be safely ignored
    fn receive_broadcast(&self, network_id: &U256, payload: &Vec<u8>) -> bool;
}

//#[derive(Debug)]
pub struct ClientConfig {
    /// Hostname to advertise as the node address, useful for DNS round robin or load balancing if wanted
    pub hostname: String,

    /// The port to listen for UDP packets on and bind to
    pub port: u16,

    /// Sets a threshold which, at sufficiently low connectivity of nodes (AKA, less than this number), new nodes will be seeked out
    pub min_nodes: u16,

    /// Sets the maximum simultaneous node connections
    pub max_nodes: u16,

    /// Synchronization servers for calculating time offset
    pub ntp_servers: Vec<String>,

    /// Endpoints to connect for a network initially if no node is available to connect to
    pub seed_nodes: Vec<NodeEndpoint>,

    /// The address used for listening (for open)
    pub bind_addr: SocketAddr,

    /// A private key used to sign and identify our own node data
    pub private_key: PKey
}

impl ClientConfig {

    /// Reccomended communication port for P2P blockscape protocol
    pub const DEFAULT_PORT: u16 = 35653;

    /// Initializes the config with reasonable defaults
    pub fn new() -> ClientConfig {
        ClientConfig::from_key(generate_private_key())
    }

    pub fn from_key(key: PKey) -> ClientConfig {
        ClientConfig {
            private_key: key,
            ntp_servers: vec!["pool.ntp.org".into()],
            seed_nodes: vec![
                NodeEndpoint {
                    host: String::from("seed-1.blockscape"),
                    port: 35653,
                    protocol: Protocol::Udp
                },
                NodeEndpoint {
                    host: String::from("seed-2.blockscape"),
                    port: 35653,
                    protocol: Protocol::Udp
                }
            ],
            min_nodes: 8,
            max_nodes: 16,
            hostname: String::from(""),
            port: ClientConfig::DEFAULT_PORT,
            bind_addr: SocketAddr::new("0.0.0.0".parse().unwrap(), ClientConfig::DEFAULT_PORT)
        }
    }
}

pub enum ClientMsg {
    GetStatistics(oneshot::Sender<Statistics>),
    GetPeerInfo(oneshot::Sender<Vec<SessionInfo>>),
    AddNode(U256, Node),
    DropNode(U256, Node),

    AttachNetwork(U256, ShardMode),
    DetachNetwork(U256),

    ShouldForge(U256, oneshot::Sender<bool>),

    SendBroadcast(U256, u8, Vec<u8>),

    RegisterBroadcastReceiver(u8, Arc<BroadcastReceiver + Send + Sync>)
}

/// Statistical information which can be queried from the network client
#[derive(Debug, Serialize, Deserialize)]
pub struct Statistics {
    /// The number of networks currently registered/working on this node
    pub attached_networks: u8,

    /// Thu number of nodes currently connected
    pub connected_peers: u32,

    /// Number of bytes received since the client started execution
    pub rx: u64,

    /// Number of bytes sent since the client started execution
    pub tx: u64,

    /// Number of milliseconds of average latency between peers
    pub avg_latency: u64
}

impl Statistics {
    fn new() -> Statistics {
        Statistics {
            attached_networks: 0,
            connected_peers: 0,
            rx: 0,
            tx: 0,
            avg_latency: 0
        }
    }
}

pub struct Client {
    /// Shared data for all network building blocks
    context: Rc<NetworkContext>,
}

impl Client {
    fn new(config: ClientConfig, rk: Arc<RecordKeeper>, core: &Core) -> Client {
        
        Client {
            context: Rc::new(NetworkContext::new(config, rk, core))
        }
    }

    fn register_broadcast_receiver(&self, id: u8, receiver: Arc<BroadcastReceiver + Send + Sync>) {
        self.context.broadcast_receivers[id as usize].set(Some(receiver));
    }

    fn udp_process_packet(&self, d: SocketPacket) -> Box<Future<Item=(), Error=io::Error>> {
        // send packet to the correct shard
        let SocketPacket(addr, p) = d;

        if p.port == 255 {
            if let Message::Introduce { node, network_id, .. } = p.payload.msg.clone() {
                // new session?
                if let Some(ref shard) = *self.context.get_shard_by_id(&network_id) {
					
					let ctx = self.context.clone();
					let node2 = node.clone();
                    let f = shard.open_session(node, None, false).then(move |r| {
						
						if let Err(_) = r {
							// TODO: Do something here
							return Err(())
						}
						
						let addr = r.unwrap();
						
						if let Some(ref shard) = *ctx.get_shard_by_id(&network_id) {
							shard.process_packet(&p.payload, &addr);
							info!("[UDP] New contact opened from {}", node2.endpoint);
						}
						
						Ok(())
					});
					
					self.context.event_loop.spawn(f);
                }
                else {
                    debug!("Invalid network ID received in join for network: {}", network_id);
                }
            }
            else {
                debug!("Received non-introduce first packet on generic port: {:?}", p);
            }
        }
        else if let Some(ref shard) = *self.context.get_shard(p.port) {

            shard.process_packet(&p.payload, &addr);
        }
        else {
            // bogus network ID received, ignore
            // TODO: A good debug print here might also print the packet
            debug!("Received unregistered network port packet: {}", p.port);
        }
        
        Box::new(future::ok(()))
    }

    fn tcp_process_packet(this: &Rc<Client>, p: Packet, stream: Box<Stream<Item=Packet, Error=io::Error>>, sink: BoxSink<Packet, io::Error>) {
        // check for introduce packet
        if let Message::Introduce { node, network_id, .. } = p.msg.clone() {
            // new session?
            let idx = this.context.resolve_port(&network_id);
            if let Some(ref shard) = *this.context.get_shard(idx) {
				
				let ctx = this.context.clone();
				let f = shard.open_session(node.clone(), None, false).then(move |r| {
					
					if let Err(_) = r {
						// TODO: Do something here
						return Err(())
					}
					
					let addr = r.unwrap();
					if let Some(ref shard) = *ctx.get_shard_by_id(&network_id) {
						shard.process_packet(&p, &addr);
						info!("[TCP] New contact opened from {}", node.endpoint);
					}

                    // handle packets
                    let t = Rc::clone(&ctx);
                    let f = stream.for_each(move |p| {
						// have to resolve the shard yet again :)
                        if let Some(ref shard) = *t.get_shard(idx) {
                            shard.process_packet(&p, &addr);
                        }

                        future::ok(())
                    }).or_else(|err| {
                        warn!("Socket decode failed: {:?}", err);
                        future::ok(())
                    });

                    ctx.event_loop.spawn(f);
					
					Ok(())
				});
				
				this.context.event_loop.spawn(f);
            }
            else {
                debug!("Invalid network ID received in join for network: {}", network_id);
            }
        }
        else {
            debug!("Received non-introduce first packet on new TCP connection: {:?}", p);
        }
    }

    /// Spawns the threads and puts the networking into a full working state
    pub fn run(config: ClientConfig, rk: Arc<RecordKeeper>, quit: QuitSignal) -> Result<(UnboundedSender<ClientMsg>, thread::JoinHandle<()>), io::Error> {        
        let (tx, rx) = unbounded::<ClientMsg>();

        let t = thread::Builder::new().name("Network Handler".into()).spawn(move || {
            info!("Network Handler thread ready");

            let mut core = Core::new().expect("Could not create network reactor core");
            let (unout, unin) = UdpSocket::bind(&config.bind_addr, &core.handle()).expect("Could not bind P2P UDP socket!")
                .framed(UDPCodec).split();

            let tnin = TcpListener::bind(&config.bind_addr, &core.handle()).expect("Could not bind P2P TCP socket!").incoming();

            let (rktx, rkrx) = mpsc::channel(10);
            rk.register_record_listener(rktx);

            let t = Rc::new(Client::new(config, rk, &core));

            t.context.sink.set(Some(Box::new(unout)));

            //let this = Rc::clone(&t);

            let this = Rc::clone(&t);
            let udp_listener = unin.for_each(move |p| {
                this.udp_process_packet(p);

                future::ok(())
            }).or_else(|e| {

                warn!("Failed to listen to packets: {}", e);

                future::err(())
            });
            
            let mut this = Rc::clone(&t);
            let tcp_listener = tnin.for_each(move |(strm, addr)| {
				debug!("Accept from {}", addr);

                // read what should be an introduce packet
                let (ttx, trx) = strm.framed(TCPCodec).split();
                let this2 = Rc::clone(&this);
                let f = trx.into_future().then(move |r| {
                    match r {
                        Ok((Some(p), trx)) => {
                            Client::tcp_process_packet(&this2, p, Box::new(trx), Box::new(ttx));
                            Ok::<(), ()>(())
                        },

                        _ => {
                            debug!("TCP Connection closed before introduce");
                            Err::<(), ()>(())
                        }
                    }

                });

                this.context.event_loop.spawn(f);

                future::ok(())
            }).or_else(|e| {

                warn!("Failed to listen to packets: {}", e);

                future::err(())
            });

            this = Rc::clone(&t);
            let msg_handler = rx.for_each(move |p| {
                let f: future::FutureResult<(), ()> = match p {
                    ClientMsg::GetStatistics(r) => future::result(r.send(this.get_stats()).map_err(|_| ())),
                    ClientMsg::GetPeerInfo(r) => future::result(r.send(this.get_peer_info()).map_err(|_| ())),
                    ClientMsg::AddNode(network_id, node) => {
                        let p = this.context.resolve_port(&network_id);
                        if p < 255  {
                            let f = this.context.get_shard(p).as_ref().unwrap()
                                .open_session(node, None, true)
                                .then(move |r| {
									// TODO: Return an actual response
									if let Err(_) = r {
										warn!("Could not add node to connection list: is the hostname correct/resolvable?");
									}
									
									Ok(())
								});
							
							this.context.event_loop.spawn(f);
                        }

                        future::ok(())
                    },
                    ClientMsg::DropNode(network_id, _) => {
                        let p = this.context.resolve_port(&network_id);
                        if p < 255 {
                            // not implemented
                        }

                        future::ok(())
                    },
                    ClientMsg::AttachNetwork(network_id, mode) => future::result(NetworkContext::attach_network(&this.context, network_id, mode).map(|_| ())),
                    ClientMsg::DetachNetwork(network_id) => {
                        this.context.detach_network(&network_id);

                        future::ok(())
                    },

                    ClientMsg::ShouldForge(network_id, r) => {
                        // must be connected to at least one node on the selected network (possibly for a period of time)
                        // must not be synchronizing

                        // this should only be called in response to a new block being processed so we can be sure that
                        // no block is forged before then, or else it is risky.
                        if let Some(ref shard) = *this.context.get_shard_by_id(&network_id) {
                            if let Err(_) = r.send(shard.get_session_count() > 0 && !NetworkJob::chain_sync_exists()) {
								// ignore
							}
                        }
                        else {
                            if let Err(_) = r.send(false) {
								// ignore
							}
                        }

                        future::ok(())
                    },

                    ClientMsg::SendBroadcast(network_id, id, payload) => {

                        // also handle the broadcast on ourselves, since broadcasts are supposed to go everywhere, including the local node
                        // if we receive a duplicate broadcast, it will automatically get stopped here as well, which is convienient
                        if !this.context.handle_broadcast(&network_id, id, &payload) {
                            return future::ok(());
                        }

                        let msg = Message::Broadcast(id, payload);

                        // get shard of ID
                        let p = this.context.resolve_port(&network_id);
                        if p < 255 {
                            this.context.get_shard(p).as_ref().unwrap().reliable_flood(msg);
                        }

                        future::ok(())
                    },

                    ClientMsg::RegisterBroadcastReceiver(id, receiver) => {
                        this.register_broadcast_receiver(id, receiver);
                        
                        future::ok(())
                    }
                };

                this.context.event_loop.spawn(f);

                future::ok(())
            });

            /*let ntpTask = Interval::new_at(Instant::now(), Duration::from_millis(NODE_NTP_INTERVAL))?
            .and_then(|_| {
                match ntp::calc_drift(this2.config.ntp_servers[0].as_str()) {
                    Ok(drift) => {
                        Time::update_ntp(drift);
                        debug!("NTP time sync completed: drift is {}", drift);
                    },
                    Err(reason) => {
                        warn!("NTP time sync failed: {}", reason);
                    }
                }
            })*/
            
            this = Rc::clone(&t);
            let session_check_task = Interval::new(Duration::from_millis(NODE_CHECK_INTERVAL), &t.context.event_loop)
                .expect("Cannot start network timer!")
                .for_each(move |_| {
                    for i in 0..255 {
                        if let Some(ref s) = *this.context.get_shard(i) {

                            debug!("Node scan started");
                            s.node_scan(this.context.config.min_nodes as usize);
                            s.check_sessions();
                        }
                    }

                    Ok(())
                })
                .or_else(|e| {
                    warn!("Failed to check sessions in timer: {}", e);

                    future::err(())
                });

            this = Rc::clone(&t);
            let rk_task = rkrx.for_each(move |e| {
                match e {
                    // otherwise do not propogate anything
                    RecordEvent::NewBlock {block, fresh: true, ..} => {
                        let p = this.context.resolve_port(&block.shard);
                        if p < 255 {
                            this.context.get_shard(p).as_ref().unwrap()
                                .reliable_flood(Message::NewBlock(block));
                        }
                        
                    },
                    // otherwise do not propogate anything
                    RecordEvent::NewTxn {txn, fresh: true} => {
                        // TODO: When we have the ability to tell which network a txn is on, apply to the correct net
                        // for now we assume genesis
                        if let Some(ref s) = *this.context.get_shard(0) {
                            s.reliable_flood(Message::NewTransaction(txn));
                        }
                        
                    },
                    _ => {}
                }

                future::ok(())
            });

            t.context.event_loop.spawn(msg_handler);
            t.context.event_loop.spawn(udp_listener);
            t.context.event_loop.spawn(tcp_listener);
            //handle.spawn(ntpTask);
            t.context.event_loop.spawn(session_check_task);
            t.context.event_loop.spawn(rk_task);

            this = Rc::clone(&t);
            core.run(quit).and_then(|_| {
                this.context.close();

                Ok(())
            }).unwrap(); // technically can never happen

            info!("Network Handler thread completed");
        }).expect("Could not start network handler thread");

        Ok((tx, t))
    }

    pub fn get_nodes_from_repo(&self, network_id: &U256, skip: usize, count: usize) -> Vec<Node> {
        let port = self.context.resolve_port(&network_id);
        if port != 255 {
            self.context.get_shard(port).borrow().as_ref().unwrap()
                .get_nodes_from_repo(skip, count);
        }

        Vec::new()
    }

    pub fn get_shard_peer_info(&self, network_id: &U256) -> Vec<SessionInfo> {

        let port = self.context.resolve_port(network_id);
        if port != 255 {
            self.context.get_shard(port).borrow().as_ref().unwrap()
                .get_session_info();
        }

        Vec::new()
    }

    pub fn get_peer_info(&self) -> Vec<SessionInfo> {

        let mut p = Vec::new();

        for i in 0..255 {
            if let Some(ref s) = *self.context.get_shard(i) {
                p.append(&mut s.get_session_info());
            }
        }

        p
    }

    pub fn get_stats(&self) -> Statistics {

        let mut stats = Statistics::new();

        for i in 0..255 {
            if let Some(ref s) = *self.context.get_shard(i) {
                stats.attached_networks += 1;
                stats.connected_peers += s.get_session_count() as u32;
            }
        }

        stats
    }

    pub fn get_config(&self) -> &ClientConfig {
        &self.context.config
    }

    pub fn get_record_keeper(&self) -> &Arc<RecordKeeper> {
        &self.context.rk
    }

    pub fn get_handle(&self) -> Handle {
        self.context.event_loop.clone()
    }
}
