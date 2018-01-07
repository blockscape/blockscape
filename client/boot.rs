use clap::{Arg, ArgGroup, ArgMatches, App};
use openssl::pkey::PKey;
use std::collections::BTreeSet;
use std::str::FromStr;
use std::net::SocketAddr;
use std::rc::Rc;

use blockscape_core::bin::Bin;
use blockscape_core::env::*;
use blockscape_core::network::client::ClientConfig;
use blockscape_core::network::node::NodeEndpoint;
use blockscape_core::primitives::*;
use blockscape_core::signer::generate_private_key;
use blockscape_core::time::Time;

use rpc::RPC;

use context::Context;

const ADMIN_KEY_PREFIX: &[u8] = b"ADMIN";
const ADMIN_KEY: &[u8] = 
b"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAyUpw2CKdIHwdHl4eTccx
gEni8PiypcXR+hQg6j5CrKc3t+WHpgQlyOz32esh+qtT4/rPAFzIAx0UNcuNMtQW
YtGSsGZW2uDA+yWX9JT221dqkgcEwezE4LxRg4iPmjOhoM/rK3JP4eHQ0QnpR9hc
uQKdDUNGnD4CIaxonOaTv6BXTm8MJrSjydRB9IguuUsZTMBCkkRsfm61MnSHHquJ
DI9tcmJxDz4RxyBsluzd4RQMUozk7X+/mwrGYaDILqNJNWV6eCWoGzmQ5qtZXx1f
vBBOiLZ1XnWuFgpL4Od8C9c2SF3IsWgrCCB2zoGxlB11hY7lDcMpPGFqZAjZne54
nQIDAQAB
-----END PUBLIC KEY-----";

/// Loads command line arguments, and returns them as a clap ArgMatches obj
pub fn parse_cmdline<'a>() -> ArgMatches<'a> {
    let workdir_arg = Arg::with_name("workdir")
        .short("w")
        .long("workdir")
        .value_name("DIR")
        .help("Sets the directory where Blockscape will store its data");

    App::new("Blockscape Official Client")
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about("P2P client and engine for the Blockscape distributed game engine")
        .group(ArgGroup::with_name("basic"))
            .arg(workdir_arg)
            .arg(Arg::with_name("disable-compute")
                .long("disable-compute")
                .help("Disables computation services for the client, making this client observe/submit only"))
            .arg(Arg::with_name("ntp-servers")
                .long("ntp-servers")
                .default_value("pool.ntp.org:123")
                .help("NTP servers to use for time correction. Must be in the format: <hostname>:<port> (default is port 123). Separated by commas."))
        .group(ArgGroup::with_name("network"))
            .arg(Arg::with_name("hostname")
                .long("host")
                .short("h")
                .help("The advertised IP or DNS host which other clients should use to connect to this client")
                .value_name("IP/HOST")
                .default_value(""))
            .arg(Arg::with_name("port")
                .long("port")
                .short("p")
                .help("Where to listen for UDP packets for P2P protocol communication")
                .value_name("NUM")
                // TODO: Better port string support for pulling directly from const, its just hard to do with the string convert
                .default_value("35653"))
            .arg(Arg::with_name("bind")
                .long("bind")
                .short("b")
                .help("IP address for interface to listen on")
                .value_name("IP")
                .default_value("0.0.0.0"))
            .arg(Arg::with_name("disable-net")
                .long("disable-net")
                .help("Disables the entire P2P interface, making the game only available for local play with no updates")
                .requires("disable-compute"))
            .arg(Arg::with_name("min-nodes")
                .long("min-nodes")
                .help("Sets the minimum number of nodes in active connection before stopping node discovery")
                .value_name("NUM")
                .default_value("8"))
            .arg(Arg::with_name("max-nodes")
                .long("max-nodes")
                .help("Sets the maximum number of nodes to allow connections for. Extra connections will be dropped")
                .value_name("NUM")
                .default_value("16"))
            .arg(Arg::with_name("seed-node")
                .long("seed-node")
                .help("Specifies the nodes to try connecting to first when none are available in the repo")
                .value_name("HOST:PORT"))
        .group(ArgGroup::with_name("rpc"))
            .arg(Arg::with_name("rpcport")
                .long("rpcport")
                .help("Set the port which the JSONRPC interface should listen on")
                .default_value("8356")
                .value_name("PORT"))
            .arg(Arg::with_name("rpcbind")
                .long("rpcbind")
                .help("Sets the interfaces which the JSONRPC interface should listen")
                .default_value("127.0.0.1")
                .value_name("HOST"))
        
        // positional argument provided means to call rpc
        .arg(Arg::with_name("rpccmd")
            .help("The JSON-RPC command to call (note: switches to rpc client mode)"))
        .arg(Arg::with_name("rpcargs")
            .help("The arguments for the RPC command")
            .multiple(true))

        .get_matches()
}

/// Returns the genesis block for blockscape
pub fn make_genesis() -> (Block, Vec<Txn>) {
    let mut b = Block {
        header: BlockHeader {
            version: 1,
            timestamp: Time::from_seconds(1508009036),
            shard: U256_ZERO,
            prev: U256_ZERO,
            merkle_root: U256_ZERO,
            blob: Bin::new(),
            creator: U160_ZERO,
            signature: Bin::new()
        },
        txns: BTreeSet::new()
    };

    let mut m = Mutation::new();

    let admkey = PKey::public_key_from_pem(ADMIN_KEY).unwrap()
        .public_key_to_der().unwrap()
        .into();

    m.changes.push(Change::SetValue {
        key: Vec::from(ADMIN_KEY_PREFIX).into(),
        value: Some(admkey),
        supp: None
    });

    let txn = Txn {
        timestamp: Time::from_seconds(1508009036),
        txn_type: 255, // special genesis block type txn
        pubkey: Vec::new(), // empty signature, not required to have one
        mutation: m,
        signature: Vec::new(),
    };

    b.txns.insert(0.into());

    // TODO: Merkle root hash happens here:
    //b.calculate_merkle_root();
    (b, vec![txn])
}

pub fn load_or_generate_key(name: &str) -> PKey {
    let key: PKey;

    if let Some(k) = load_key(name) {
        key = k;
        info!("Loaded node keyfile from file.");
    }
    else {
        info!("Generate node keyfile...");
        // need to create key
        key = generate_private_key();

        // save the key (fail if not saved)
        if !save_key(name, &key) {
            panic!("Could not save node private key file.");
        }
    }

    key
}

/// Converts the command line arguments to a client config ready to go
/// # Arguments
/// * `cmdline`: The argument matches from clap on the command line
/// *Note*: As this is a high level function, it will automatically try to load the network key from
/// file, and it will generate a new one if needed
/// # Panics
/// If it cannot save a newly created public key, or if the private key loaded is invalid
pub fn make_network_config(cmdline: &ArgMatches) -> ClientConfig {

    let key = load_or_generate_key("node");

    let mut config = ClientConfig::from_key(key);

    config.min_nodes = cmdline.value_of("min-nodes").unwrap().parse::<u16>().expect("Invalid value for min-nodes: must be a number!");
    config.max_nodes = cmdline.value_of("max-nodes").unwrap().parse::<u16>().expect("Invalid value for max-nodes: must be a number!");
    config.ntp_servers = cmdline.value_of("ntp-servers").unwrap().split(',').map(|s| String::from(s)).collect();
    config.hostname = String::from(cmdline.value_of("hostname").unwrap());
    config.port = cmdline.value_of("port").unwrap().parse::<u16>().expect("Invalid P2P port: must be a number!");
    if cmdline.is_present("seed-node") {
        config.seed_nodes = cmdline.values_of_lossy("seed-node").unwrap().iter()
            .map(|x| NodeEndpoint::from_str(x)
                .expect(format!("Invalid hostname for seed node: {}. Did you include the port?", x).as_str()))
            .collect();
    }

    config.bind_addr = SocketAddr::new(cmdline.value_of("bind").unwrap().parse().expect("Invalid bind IP"), config.port);

    config
}

/// Starts the JSONRPC server
pub fn make_rpc(cmdline: &ArgMatches, ctx: Rc<Context>) -> RPC {

    let bind_addr = SocketAddr::new(cmdline.value_of("rpcbind").unwrap().parse().expect("Invalid RPC bind IP"), 
            cmdline.value_of("rpcport").unwrap().parse::<u16>().expect("Invalid RPC port: must be a number!"));

    RPC::run(bind_addr, ctx)
}

pub fn call_rpc(cmdline: &ArgMatches) -> bool {

    use rpc::client::JsonRpcRequest;

    let method = cmdline.value_of_lossy("rpccmd").expect("Unknown encoding for RPC command!").into_owned();
    let args = cmdline.values_of_lossy("rpcargs").unwrap_or(Vec::new());

    debug!("Calling RPC: {}", method);

    let bind_addr = SocketAddr::new(cmdline.value_of("rpcbind").unwrap().parse().expect("Invalid RPC bind IP"), 
            cmdline.value_of("rpcport").unwrap().parse::<u16>().expect("Invalid RPC port: must be a number!"));

    let res = JsonRpcRequest::new(method, args).exec_sync(bind_addr);

    if res.is_err() {
        println!("RPC Error: {}", res.err().unwrap());

        false
    }
    else {
        println!("{}", res.unwrap());

        true
    }
}