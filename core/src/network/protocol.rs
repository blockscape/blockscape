use std::sync::Arc;
use std::net::SocketAddr;
use std::rc::Rc;

use openssl::pkey::PKey;

use futures::prelude::*;
use futures::future;
use record_keeper::{Error, LogicError, BlockchainEntry, Key, BlockPackage};
use bin::Bin;
use time::Time;
use signer::*;
use primitives::{U256,Block,Txn};

use network::job::{NetworkJob, NetworkJobData};
use network::node::Node;
use network::session::{Session,GenericSession};
use network::shard::ShardInfo;

pub const PROTOCOL_VERSION: u16 = 1;

/// How much of the ping value to retain. The current value keeps a weighted average over 10 minutes
pub const PING_RETENTION: f32 = 40.0;

/// Number of milliseconds to wait before declaring a ping failed
pub const PING_TIMEOUT: i64 = 3000;

/// Number of milliseconds to wait before declaring a job failed
pub const JOB_TIMEOUT: i64 = 5000;

/// The number of strikes which may accumulate before declaring the connection timed out
pub const TIMEOUT_TOLERANCE: u64 = 3;

/// The number of nodes which should be sent back on a list node request
pub const NODE_RESPONSE_SIZE: usize = 8;


//const NODE_SCAN_INTERVAL: u64 = 30000; // every 30 seconds
pub const NODE_CHECK_INTERVAL: u64 = 5000; // every 5 seconds
//const NODE_NTP_INTERVAL: u64 = 20 * 60000; // every 20 minutes

/// The maximum amount of data that can be in a single message object (the object itself can still be in split into pieces at the datagram level)
/// Right now it is set to 64k, which is the highest number supported by kernel fragmentation right now.
pub const MAX_PACKET_SIZE: usize = 64 * 1000;

pub const MAX_JOB_RETRIES: usize = 3;

pub const MAX_ABUSES: usize = 3;

pub struct SocketPacket(pub SocketAddr, pub RawPacket);

/// A packet as it appears before being assigned to a network. Networks are assigned by "port". Only used for UDP connections
/// because TCP connections are inheretly already associated with a particular session object
#[derive(Serialize, Deserialize, Debug)]
pub struct RawPacket {
    /// Which communication channel should be regarded for this node.
    /// This is included so nodes can have multiple connections to each other through separate shards
    /// Port 255 is reserved for connecting from remote nodes when the local port is unknown
    pub port: u8,
    /// The data which should be delivered to the session handler
    pub payload: Packet
}

/// An enclosed Message structure, including additional message pertinent options as needed
#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    /// A numerical ID to distinguish this sent packet from others
    pub seq: u32,
    /// The actual payload
    pub msg: Message,
    // optional signature to verify the message is genuine, sometimes required for certian sensitive operations
    sig: Bin
}

impl Packet {

    pub fn new(seq: u32, msg: Message) -> Packet {
        Packet {
            seq,
            msg,
            sig: Bin::new()
        }
    }

    /// Ensures that the packet is signed and is correct for the encoded message
    pub fn check_sig(&self, node: &Node) -> bool {

        let r = PKey::public_key_from_der(&node.key);

        if let Ok(key) = r {
            verify_obj(&self.msg, &self.sig, &key)
        }
        else if let Err(e) = r {
            warn!("Remote key {} could not be decoded from DER: {:?}", node.get_hash_id(), e);
            false
        }
        else {
            unreachable!();
        }
    }

    /// Add a signature to the message using the given assymetric key
    pub fn apply_sig(mut self, key: &PKey) -> Packet {
        self.sig = sign_obj(&self.msg, key);

        self
    }
}

/// The payload of a packet, determines the operations to take
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    /// First message sent by a connecting node. If the other node accepts, it will reply with an "Introduce". The nodes are now connected
    Introduce {
        /// The network which this node is attempting to make a connection through
        network_id: U256,
        /// Information about the node that is connecting
        node: Node,
        /// The port which should be used for future packets to this node for this network
        port: u8
    },                   

    /// Sent to check connection status with client
    Ping(Time),
    /// Sent to reply to a previous connection status request
    Pong(Time),

    /// Sent when a node would like to query peers of another node, in order to form more connections to the network
    FindNodes {
        /// Regardless of whatever network ID may be associated with a session, this property defines which network to return packets of
        network_id: U256,
        /// If more nodes are needed, an offset can be indicated here to get additional nodes with subsequent queries
        skip: u16
    },

    /// In reply to FindNodes, to indicate nodes which can be connected to
    NodeList {
        /// A list of nodes which can be connected to. An empty list means there is no more data here
        nodes: Vec<Node>,
        /// The original requested network id
        network_id: U256,
        /// If more nodes are needed, an offset can be indicated here to get additional nodes with subsequent queries (just increment by however many you received)
        /// Currently this is not used for anything.
        skip: u16
    },

    /// Sent by reliable flooding to indicate a new transaction has entered the network and should be propogated
    NewTransaction(Txn),
    /// Sent by reliable flooding to indicClientate that a new block has entered the network and should be propogated
    NewBlock(Block),

    /// Sent by reliable flooding to send messages between all connected nodes of the DApp.
    /// NOTE: Please do not use this to settle any consensus! That should be placed in a txn in the blockchain. It is only a tool to *lead to* consensus (ex. to pass signatures/secrets).
    Broadcast(u8, Vec<u8>),

    /// Request block synchronization data, starting from the given block hash, proceeding to the last block hash
    SyncBlocks { last_block_hash: U256, target_block_hash: U256 },
    /// Request specific block or transaction data as indicated by the list of hashes given
    QueryData(Vec<U256>),
    /// Returned in response to a request for txn/block data (either SyncBlocks or QueryData) to provide bulk data to import from the blockchain
    /// The first U256 indicates the hash of the block this package will fulfill, giving the sync process an opporitunity to continue downloading
    /// from where the package left off without needing to decompress it.
    ChainData(U256, Vec<u8>),

    /// In the case that a node somehow missed some individual piece of data (like a single txn), this function is used to send it
    SpotChainData(Vec<Block>, Vec<Txn>),

    /// Sent to signal the end of the connection
    Bye(ByeReason),

    /// Sent when a previous call to QueryData or SyncBlocks is not able to be fulfilled, in whole or in part.
    DataError(DataRequestError)
}

/// Sent when data is not able to returned for some reason
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DataRequestError {
    /// The requested hash does not exist on this node
    HashesNotFound(Vec<U256>),
    /// Too many requests have come from your node to be processed in quick succession
    RateExceeded,
    /// This node is not an authoritative source for information on the requested shard ID
    NetworkNotAvailable,
    /// Service could not be provided because of an error within the contacted node which is not believed to be related to the request itself
    InternalError
}

/// Sent when a client is ending the connection and additional future information as needed
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum ByeReason {
    /// Node is simply disconnecting
    Exit,
    /// Connection should not be attempted to this node again because the node is incompatible or wrong
    /// For example, connecting to self.
    ExitPermanent,
    /// Node has not been responding, or no longer seems to be available
    Timeout,
    /// Node has not been nice, so disconnect
    Abuse
}

impl Packet {
    /// Try to auttomatically handle the packet. May interact with the session depending on contents
    /// if the packet cannot be handled directly from the session (i.e. is a job result), 
    pub fn handle(&self, sess: &Rc<GenericSession>, shard: &ShardInfo) -> Option<()> {
        // handle all of the different packet types
        match self.msg {
            Message::Introduce { .. } => {
                if !self.check_sig(&sess.get_remote_node()) {
                    //.done.set(Some(ByeReason::ExitPermanent));
                    sess.close();
                    return Some(());
                }

                // reply to the introduce since sometimes the 
                sess.send_introduce();

                Some(())
            }

            Message::Ping(time) => {
                // Send back a pong
                sess.send_reply(Message::Pong(time), self.seq, false);
                Some(())
            },

            Message::Pong(time) => {
                // save ping information
                sess.recv_ping(time);
                Some(())
            },

            Message::FindNodes { ref network_id, ref skip } => {

                let nodes = shard.get_session_info().into_iter()
                    .skip(*skip as usize)
                    .take(NODE_RESPONSE_SIZE as usize)
                    .filter_map(|p| {
                        if p.peer == *sess.get_remote_node() {
                            None
                        }
                        else {
                            Some(p.peer)
                        }
                    })
                    .collect();

                sess.send_reply(Message::NodeList {
                        nodes: nodes,
                        network_id: network_id.clone(),
                        skip: skip.clone()
                }, self.seq, false);

                Some(())
            },

            Message::NewTransaction(ref txn) => {
                let d = txn.clone();
                let rk = Arc::clone(&sess.get_context().rk);
                sess.get_context().event_loop.spawn(sess.get_context().rk.get_worker().spawn_fn(move || {
                    rk.add_pending_txn(d, true)
                }).map(|_| ()).or_else(|err| {
                    // react for this node's records here if they are bad
                    match err {
                        Error::NotFound(_) => {
                            // submit a new job
                            // TODO: currently undefined/should not happen, so what
                        },
                        Error::Logic(_e) => {
                            // TODO: Mark on node record and kick
                        },
                        _ => {
                            // TODO: most likely some internal error occured, but where?
                        }
                    }

                    Ok::<(), ()>(())
                }));

                Some(())
            },

            Message::NewBlock(ref block) => {
                let d = block.clone();
                let d2 = block.clone();
                let rk = Arc::clone(&sess.get_context().rk);
                let lcontext = Rc::clone(&sess.get_context());
                let network_id = sess.get_network_id().clone();
                sess.get_context().event_loop.spawn(sess.get_context().rk.get_worker().spawn_fn(move || {
                    rk.add_block(&d, true)
                }).map(|_| ()).or_else(move |err| {
                    // react for this node's records here if they are bad
                    match err {
                        Error::NotFound(Key::Blockchain(missing_obj)) => {
                            match missing_obj {
                                BlockchainEntry::BlockHeader(_hash) => {

                                    // get the current chain head
                                    let current_head = lcontext.rk.get_current_block_hash();

                                    // new job to find this block
                                    let job = NetworkJob::new(NetworkJobData::SyncChain(d2, current_head));

                                    if let Some(ref shard) = *lcontext.get_shard_by_id(&network_id) {
                                        shard.assign_job(job);
                                    }

                                    // TODO: set a new work target
                                    /*if let Err(e) = lcontext.job_targets.unbounded_send(
                                        // TODO: Move out reference to record keep,r which could stall network thread!
                                        (NetworkJob::new(network_id, incoming_hash, lcontext.rk.get_current_block_hash(), None), Some(prev))
                                    ) {
                                        // should never happen
                                        warn!("Could not buffer new network job: {}", e);
                                    };*/
                                },
                                BlockchainEntry::Txn(_hash) => {
                                    // TODO: request a single txn from some node so we can get patched up
                                },
                                BlockchainEntry::TxnList(_hash) => {
                                    // should never happen
                                    panic!("Database is missing an entire txn list! Should never happen.");
                                }
                            }
                        },
                        Error::Logic(_e) => {
                            // TODO: Mark on node record and kick
                        },
                        _ => {
                            // TODO: most likely some internal error occured, but where?
                        }
                    }

                    Ok::<(), ()>(())
                }));

                Some(())
            },

            Message::Broadcast(ref id, ref payload) => {
                if !sess.get_context().handle_broadcast(&sess.get_network_id(), *id, payload) {
                    sess.mark_abuse();
                }

                Some(())
            },

            Message::SyncBlocks { ref last_block_hash, ref target_block_hash } => {
                // generate a block package from the db
                let lbh = last_block_hash.clone();
                let tbh = target_block_hash.clone();
                let rk = Arc::clone(&sess.get_context().rk);
                let wsess = Rc::downgrade(sess);
                let seq = self.seq;
                sess.get_context().event_loop.spawn(sess.get_context().rk.get_priority_worker().spawn_fn(move || {
                    let bp = rk.get_blocks_between(&lbh, &tbh, MAX_PACKET_SIZE)?;

                    if bp.is_empty() {
                        return Err(Error::Logic(LogicError::Duplicate));
                    }

                    let lh = bp.last_hash();
                    Ok((lh, bp.zip()?))
                })
                .then(move |r| {

                    if let Ok((to, d)) = r {
                        // send back

                        if let Some(sess) = wsess.upgrade() {
                            sess.send_reply(Message::ChainData(to, d), seq, true);
                        }
                    }
                    else {
                        match r.unwrap_err() {
                            Error::NotFound(Key::Blockchain(missing_obj)) => {
                                let h = match missing_obj {
                                    BlockchainEntry::BlockHeader(hash) => hash,
                                    BlockchainEntry::Txn(hash) => hash,
                                    BlockchainEntry::TxnList(hash) => hash
                                };

                                if let Some(sess) = wsess.upgrade() {
                                    sess.send_reply(Message::DataError(DataRequestError::HashesNotFound(vec![h])), seq, true);
                                }
                            },

                            _ => {
                                // no idea what happened
                                if let Some(sess) = wsess.upgrade() {
                                    sess.send_reply(Message::DataError(DataRequestError::InternalError), seq, true);
                                }
                            }
                        }
                    }

                    Ok::<(), ()>(())
                }));

                Some(())
            },

            Message::QueryData(ref hashes) => {
                let d = hashes.clone();
                // get stuff form the db
                let rk = Arc::clone(&sess.get_context().rk);
                let wsess = Rc::downgrade(sess);
                let seq = self.seq;
                sess.get_context().event_loop.spawn(sess.get_context().rk.get_priority_worker().spawn_fn(move || {
                    let mut blocks: Vec<Block> = Vec::new();
                    let mut txns: Vec<Txn> = Vec::new();

                    let mut failed: Vec<U256> = Vec::new();

                    for hash in d {
                        if let Ok(txn) = rk.get_txn(&hash) {
                            txns.push(txn);
                        }
                        else if let Ok(block) = rk.get_block(&hash) {
                            blocks.push(block);
                        }
                        else {
                            failed.push(hash.clone());
                        }
                    }

                    Ok((blocks, txns, failed))

                }).and_then(move |(blocks, txns, failed)| {
                    if !blocks.is_empty() || !txns.is_empty() {
                        if let Some(sess) = wsess.upgrade() {
                            sess.send_reply(Message::SpotChainData(blocks, txns), seq, true);
                        }
                    }
                    if !failed.is_empty() {
                        if let Some(sess) = wsess.upgrade() {
                            sess.send_reply(Message::DataError(DataRequestError::HashesNotFound(failed)), seq, true);
                        }
                    }

                    Ok::<(), ()>(())
                }));

                Some(())
            },

            Message::SpotChainData(ref _blocks, ref _hashes) => {
                // TODO: Fill someday
                Some(())
            },

            Message::ChainData(ref to, ref raw_pkg) => {

                let lcontext = Rc::clone(&sess.get_context());
                let rk = Arc::clone(&sess.get_context().rk);
                let rk2 = Arc::clone(&sess.get_context().rk);
                let raw_pkg = raw_pkg.clone();
                let to = to.clone();
                let f1 = sess.get_context().rk.get_priority_worker().spawn_fn(move || {
                    BlockPackage::unzip(&raw_pkg)
                        .map_err(|e| (e, rk.get_current_block_hash()))
                });

                let f2 = f1.then(move |res| {
                    match res {
						// TODO: Use size
                        Ok((pkg, _size)) => {

                            if pkg.is_empty() {
                                warn!("Received empty block package from peer!");
                                return future::err(());
                            }

                            if pkg.last_hash() != to {
                                warn!("Hash of last block in package does not match original index provided");
                                // TODO: Mark this peer for abuse
                            }

                            // now that we have unpacked, actually import the data
                            let f = lcontext.rk.get_priority_worker().spawn_fn(move || {
                                rk2.import_pkg(pkg).map_err(|e| (e, rk2.get_current_block_hash()))
                            }).then(move |res| {
                                match res {
									// TODO
                                    Ok(_imported_to) => {
                                        Ok::<(), ()>(())
                                    },
                                    Err((err, _cbh)) => {
                                        // react for this node's records here if they are bad
                                        match err {
                                            Error::Deserialize(_e) => {
                                                // TODO: Mark on node record and kick
                                            },
                                            Error::Logic(_e) => {
                                                // TODO: Mark on node record and kick

                                            }
                                            _ => {
                                                // TODO: most likely some internal error occured, but where?
                                            }
                                        }

                                        Ok::<(), ()>(())
                                    }
                                }
                            });

                            lcontext.event_loop.spawn(f);
                        },
                        Err((err, _cbh)) => {
                            // react for this node's records here if they are bad
                            match err {
                                Error::Deserialize(_e) => {
                                    // TODO: Mark on node record and kick
                                },
                                Error::Logic(_e) => {
                                    // TODO: Mark on node record and kick

                                }
                                _ => {
                                    // TODO: most likely some internal error occured, but where?
                                }
                            }
                        }
                    }

                    future::ok(())
                });

                sess.get_context().event_loop.spawn(f2);

                Some(())
            },

            Message::DataError(ref err) => {
                // data could not be requested: does this have to do with our currently active job?
                match *err {
                    DataRequestError::HashesNotFound(ref _hashes) => {
                        unimplemented!();
                        
                    }
                    _ => {
                        // TODO:
                        // currently unimplemented
                    }
                }

                Some(())
            },

            Message::Bye(ref _reason) => {
                // remote end has closed the connection, no need to reply, just mark this session as that reason
                sess.close();

                Some(())
            },

            _ => None
        }
    }
}
