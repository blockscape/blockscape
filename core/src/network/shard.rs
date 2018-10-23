use std::cell::*;
use std::io;
use std::cmp::min;
use rand;
use rand::Rng;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::ops::Deref;
use std::rc::Rc;

use futures::prelude::*;
use futures::future;
use futures::sink::BoxSink;
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;

use network::context::*;
use network::job::*;
use network::node::{Node, NodeRepository, Protocol};
use network::protocol::{Message, ByeReason, Packet, MAX_JOB_RETRIES};
use network::session::{GenericSession, Session, SessionInfo, NewSessionOptions};
use network::tcp::TCPCodec;
use primitives::{U256, U160_ZERO, U160};

/// Defines the kind of interaction this node will take with a particular shard
pub enum ShardMode {
    /// Full participation, operating in block mining, full work processing, full authority
    Primary,
    /// This is a long term connection and we still validate and sync on this shard, but less processing, primarily just validation
    Auxillery,
    /// Used when connecting to a shard to only get information from authoritative network sources. Good for when a player views a arbitrary shard
    /// for gameplay. 
    QueryOnly
}

pub struct ShardInfo {

    /// The network context
    context: Rc<NetworkContext>,

    /// Unique identifier for this shard (usually genesis block hash)
    network_id: U256,

    /// The port which should be assigned to clients
    pub port: u8,

    /// Functional requirements of this shard connection
    pub mode: ShardMode,

    /// Independant "connections" to each of the other NodeEndpoints we interact with
    sessions: RefCell<HashMap<SocketAddr, Rc<GenericSession>>>,

    peer_ids: RefCell<HashSet<U160>>,

    /// The index of the node we should scan next in the node repository. Incremented for each connection attempt
    last_peer_idx: Cell<usize>,

    /// Collection of nodes which can be connected to this on this network
    node_repo: RefCell<NodeRepository>,
    
    /// If no nodes are connected, the broadcast could be lost before it reaches another node.
    /// We store unsent broadcasts here so we can ensure they are eventually sent.
    unsent_broadcasts: RefCell<Vec<Message>>
}

impl ShardInfo {
    pub fn new(network_id: U256, port: u8, mode: ShardMode, context: Rc<NetworkContext>, repo: NodeRepository) -> ShardInfo {
        ShardInfo {
            context: context,
            network_id: network_id,
            port: port,
            mode: mode,
            sessions: RefCell::new(HashMap::new()),
            peer_ids: RefCell::new(HashSet::new()),
            last_peer_idx: Cell::new(0),
            node_repo: RefCell::new(repo),
            unsent_broadcasts: RefCell::new(Vec::new())
        }
    }

    /// Try to scan the node database and ensure the minimum requested number of nodes
    pub fn node_scan(&self, wanted: usize) -> usize {

        let cur_count = self.sessions.borrow().len();

        if cur_count >= wanted {
            return cur_count; // no need to do any more
        }

        // Ask a random peer for more nodes, to keep the database saturated
        {
            let s = self.sessions.borrow();
            if !s.is_empty() {
                let mut rng = rand::thread_rng();
                let sess = rand::sample(&mut rng, s.values(), 1)[0];

                // we do not care if the job failed to assign for right now
                if !sess.assign_job(&NetworkJob::new(NetworkJobData::FindNodes(self.network_id.clone()))) {
                    debug!("Failed to assign network scan to node (most likely occupied)");
                }
            }
        }

        {
            let nrepo = self.node_repo.borrow();
            let mut queue = Vec::new();

            //info!("Starting new node queue size: {}, repo size: {}", queue.len(), nrepo.len());

            // do we need more nodes to connect to (from the queue)? If so, pull from the node repo
            let mut attempts = 0;
            if nrepo.len() > 0 {
                while cur_count + queue.len() < wanted {
                    let peer = nrepo.get_nodes(self.last_peer_idx.get());
                    self.last_peer_idx.set(self.last_peer_idx.get() + 1);

                    if !self.peer_ids.borrow().contains(&peer.get_hash_id()) && peer.get_hash_id() != self.context.my_node.get_hash_id() {
                        queue.push(peer);
                    }

                    attempts += 1;

                    if attempts >= wanted * 3 || attempts >= nrepo.len() {
                        break;
                    }
                }
            }

            debug!("Reaching for {} new nodes...", queue.len());

            // pull from the connection queue

            for peer in queue.into_iter() {
				
				let ctx = Rc::clone(&self.context);
				let network_id = self.network_id.clone();
				let local_peer = peer.clone();
				let f = self.open_session(peer.clone(), None, true)
				.then(move |r| {
					if let Ok(sopt) = r {
						info!("New contact opened to {} ({})", local_peer.endpoint, sopt);
					}
					else if let Some(ref shard) = *ctx.get_shard_by_id(&network_id) {
						// remove node because it is broken DNS
						let mut repo = shard.node_repo.borrow_mut();
						debug!("Remove broken node from repo: {:?}", r);
						repo.remove(&local_peer.get_hash_id());

						// TODO: debounce this
						let r = repo.save(format!("{}", network_id).as_str());

						if r.is_err() {
							warn!("Failed to save nodes to file: {:?}", r.unwrap_err());
						}
						else {
							debug!("Saved {} nodes from repo", r.unwrap());
						}
					}
					
					Ok(())
				});
				
				self.context.event_loop.spawn(f);
            }
        }

        self.sessions.borrow().len()
    }

    /// Sends pings and removes dead connections as necessary
    pub fn check_sessions(&self) {

        let mut jobs: Vec<NetworkJob> = Vec::new();

        {
            let mut removed: Vec<SocketAddr> = Vec::new();

            let mut s = self.sessions.borrow_mut();
            let mut pids = self.peer_ids.borrow_mut();

            pids.clear();

            for (addr, sess) in s.iter_mut() {
                sess.check_conn();
                
                if let Some(mut j) = sess.check_job() {
                    // job failed, try to gracefully reassign
                    j.try.set(j.try.get() + 1);

                    if j.try.get() > MAX_JOB_RETRIES {
                        warn!("Failed job, dropping: {:?}", j);
                    }
                    else {
                        jobs.push(j);
                    }
                }

                if let Some(d) = sess.is_done() {
                    
                    removed.push(addr.clone());

                    let rn = sess.get_remote_node();

                    debug!("Remove session: {:?}", rn.endpoint);
                    // may have to do something additional depending on the failure reason:
                    match d {
                        ByeReason::ExitPermanent => {
                            // remove this node from the db as well
                            self.node_repo.borrow_mut().remove(
                                &rn.get_hash_id()
                            );
                        },
                        // TODO: Maybe also add the node to a blacklist of some kind?
                        ByeReason::Abuse => {
                            // remove this node from the db as well
                            self.node_repo.borrow_mut().remove(
                                &rn.get_hash_id()
                            );
                        },
                        _ => {}
                    }
                }
                else {
                    pids.insert(sess.get_remote_node().get_hash_id());
                }

                // TODO: for now this is a little inefficient (requires a U160 hash for each client every 5 seconds), but it works
                // add introduced nodes to the repo
                if sess.is_introduced() && self.node_repo.borrow().get(&sess.get_remote_node().get_hash_id()).is_none() {
                    debug!("Add node to DB: {:?}", sess.get_remote_node().get_hash_id());
                    self.node_repo.borrow_mut().new_node(
                        sess.get_remote_node().clone()
                    );
                }
            }

            for remove in removed {
                let sess = s.remove(&remove).unwrap();

                // make sure there are no outstanding references (assertion)
                if Rc::try_unwrap(sess).is_err() {
                    panic!("multiple strong references to a session should not exist");
                }
            }
        }

        for job in jobs {
            self.assign_job(job);
        }
    }

    pub fn open_session(&self, peer: Node, strm: Option<BoxSink<Packet, io::Error>>, introduce: bool) -> Box<Future<Item=SocketAddr, Error=io::Error>> {

        // check matching remote endpoints
        if self.sessions.borrow().values().find(|s| s.get_remote_node().endpoint == peer.endpoint).is_some() {
            // already connected
            return Box::new(future::err(io::Error::new(io::ErrorKind::AlreadyExists, "Already connected on same network_id")));
        }

        // check matching peer ids
        let hid = peer.get_hash_id();
        if hid != U160_ZERO {
            if self.sessions.borrow().values().find(|s| s.get_remote_node().get_hash_id() == hid).is_some() {
                // already connected
                return Box::new(future::err(io::Error::new(io::ErrorKind::AlreadyExists, "Same peer id connected on same network_id")));
            }
        }
        
        let strm_is_none = strm.is_none();
        
        // lets get the peer address resolved
        let ctx = Rc::clone(&self.context);
        let network_id = self.network_id.clone();
        let port = self.port;
        Box::new(peer.endpoint.clone().as_socketaddr().then(move |r| {
			if r.is_err() {
				return r;
			}
			
			let addr = r.unwrap();
			
			let mut opts = NewSessionOptions {
				context: Rc::clone(&ctx), 
				local_port: port, 
				remote_peer: peer.clone(), 
				remote_addr: addr.clone(),
				network_id: network_id, 
				sink: strm
			};
			
			if peer.endpoint.protocol == Protocol::Tcp && strm_is_none {
				let ctx2 = Rc::clone(&ctx);
				ctx.event_loop.spawn(TcpStream::connect(&addr.clone(), &ctx.event_loop).then(move |r| {
					match r {
						// TODO: Something better than a warning here would be nice
						Err(err) => warn!("Could not open TCP connection to peer: {:?}", err),
						Ok(s) => {
							let (ttx, trx) = s.framed(TCPCodec).split();
							
							opts.sink = Some(Box::new(ttx));
							
							let sess = GenericSession::new(opts);
							
							if let Some(ref shard) = *ctx2.get_shard_by_id(&network_id) {
								if shard.sessions.borrow().contains_key(&addr) {
									// already connected
									return Err(())
								}

								// we have to send an introduce if we were not given a socket, logically
								sess.send_introduce();

								shard.peer_ids.borrow_mut().insert(sess.get_remote_node().get_hash_id());
								shard.sessions.borrow_mut().insert(addr.clone(), Rc::new(sess));
								
								let ctx3 = Rc::clone(&ctx2);
							
								ctx2.event_loop.spawn(trx.for_each(move |p| {
									if let Some(ref shard) = *ctx3.get_shard(port) {
										shard.process_packet(&p, &addr);
									}

									future::ok(())
								}).or_else(|err| {
									warn!("Socket decode failed: {:?}", err);
									future::ok(())
								}));
							}
							else {
								warn!("Shard appears to be closed. Cancelling session...");
								sess.close();
							}
						}
					};
					
					Ok(())
				}));
				
				Ok(addr)
			}
			else {
				// ready to connect
				let sess = GenericSession::new(opts);

				if introduce {
					sess.send_introduce();
				}

				if let Some(ref shard) = *ctx.get_shard_by_id(&network_id) {
					shard.peer_ids.borrow_mut().insert(sess.get_remote_node().get_hash_id());
					shard.sessions.borrow_mut().insert(addr.clone(), Rc::new(sess));
				}
				else {
					warn!("Shard appears to be closed. Cancelling session...");
					sess.close();
				}

				Ok(addr)
			}
		}))
    }

    /// Try to give the provided job to a randomly selected node in the network
    pub fn assign_job(&self, job: NetworkJob) -> bool {
        let s = self.sessions.borrow();
        let mut rng = rand::thread_rng();
        let mut pulls: Vec<&Rc<GenericSession>> = s.values().collect();
        rng.shuffle(&mut pulls);

        for pull in pulls {
            if pull.assign_job(&job) {
                return true;
            }
        }

        false
    }

    /// Call to set this shard to a state where all nodes are disconnected and data should stop being validated/tracked
    pub fn close(&self) {
        debug!("Close shard: {}", self.network_id);
        for sess in self.sessions.borrow_mut().values_mut() {
            sess.close();
        }
    }

    /// Evaluate a single packet and route it to a session as necessary
    pub fn process_packet(&self, p: &Packet, addr: &SocketAddr) {
        
        // debug logging
        match p.msg {
            Message::Ping { .. } => {},
            Message::Pong { .. } => {},
            Message::NewBlock(ref b) => debug!("Import block {}", b.calculate_hash()),
            Message::NewTransaction(ref t) => debug!("Import txn {}", t.calculate_hash()),
            Message::Introduce { ref node, .. } => debug!("{} ==> Introduce {}", addr, node.get_hash_id()),
            Message::NodeList { .. } => {},
            Message::FindNodes {..} => {},
            Message::ChainData(ref to, ref data) => debug!("Received {} bytes of chain data to block {}", data.len(), to),
            _ => debug!("{} ==> {:?}", addr, &p)
        };

        // special case can happen if UDP packet routing leads to a narrower connection path
        if let Message::Introduce {ref node, ..} = p.msg {
            let hid = node.get_hash_id();
            
            // find it
            let mut sessions = self.sessions.borrow_mut();
            let mut f = None;

            // first, look at the source addr
            if sessions.contains_key(addr) {
                f = Some(addr.clone());
            }
            // we might also find it as a pre-initialized session
            else if let Some((sa, _)) = sessions.iter_mut().find(|&(_,ref v)| v.get_remote_node().get_hash_id() == hid) {
                f = Some(sa.clone());
            }
            else {
                // could not find
                warn!("Unroutable introduce packet: {:?}", node.get_hash_id());
            }

            if let Some(sa) = f {

                if let Ok(sess) = Rc::try_unwrap(sessions.remove(&sa).unwrap()) {
                    let sess = sess.handle_introduce(p, addr);
                    info!("Remote peer {} has changed connection configuration!", hid);
                    
                    let mut unsent_broadcasts = self.unsent_broadcasts.borrow_mut();
                    
                    // drain our broadcasts on this node
                    for msg in unsent_broadcasts.drain(..) {
                        sess.send(msg, true);
                    }

                    sessions.insert(sess.get_remote_addr().clone(), Rc::new(sess));
                }
                else {
                    panic!("multiple strong references of session should not exist");
                }
            }
        }
        else {

            let mut job = None;

            // load sessions in a separate context, in case we need to add 
            // a new session because of a job
            {
                let sessions = self.sessions.borrow();

                if let Some(sess) = sessions.get(&addr) {
                    job = GenericSession::recv(sess, p, self);
                }
                else {
                    warn!("Unroutable packet: {:?}", p);
                }   
            }

            // process/react to the job
            if let Some(j) = job {
                if let Some(newjob) = j.complete(&p.msg, &self.context) {
                    self.assign_job(newjob);
                }
            }
        }
    }

    pub fn reliable_flood(&self, msg: Message) {
        
        let sessions = self.sessions.borrow();
        if sessions.is_empty() {
            self.unsent_broadcasts.borrow_mut().push(msg);
        }
        else {
            sessions.values().for_each(|s| {
                if s.is_introduced() && s.is_done().is_none() {
                    s.send(msg.clone(), true);
                }
            });
        }
    }

    pub fn get_network_id(&self) -> &U256 {
        return &self.network_id;
    }

    pub fn get_session_count(&self) -> usize {
        // filter only sessions which are past introductions
        let mut count = 0;
        for sess in self.sessions.borrow().values() {
            if sess.is_introduced() {
                count += 1;
            }
        }

        count
    }

    /// Returns information on all active sessions
    pub fn get_session_info(&self) -> Vec<SessionInfo> {

        let mut vec = Vec::new();

        for sess in self.sessions.borrow().values() {
            if sess.is_introduced() {
                vec.push(sess.get_info());
            }
        }

        vec
    }

    pub fn get_nodes_from_repo(&self, skip: usize, count: usize) -> Vec<Node> {

        let nrepo = self.node_repo.borrow();

        let mut nodes: Vec<Node> = Vec::with_capacity(min(count, nrepo.len()));

        for i in skip..min(nrepo.len(), skip + count) {
            // dont send the node if it is self
            let d = nrepo.get_nodes((skip + i) as usize);
            let n = d.deref();

            nodes.push(n.clone());
        }

        nodes
    }
}
