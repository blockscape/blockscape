use clap::{Arg, ArgGroup, ArgMatches, App};
use openssl::pkey::PKey;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read,Write};
use std::str::FromStr;
use std::net::SocketAddr;

use blockscape_core::env::*;
use blockscape_core::network::client::ClientConfig;
use blockscape_core::network::node::NodeEndpoint;
use blockscape_core::primitives::*;
use blockscape_core::signer::generate_private_key;
use blockscape_core::time::Time;

const ADMIN_KEY_PREFIX: &[u8] = b"ADMIN";
const ADMIN_KEY: &[u8] = b""; //TODO: Insert Admin Key

/// Loads command line arguments, and returns them as a clap ArgMatches obj
pub fn parse_cmdline<'a>() -> ArgMatches<'a> {
    let workdir_arg = Arg::with_name("workdir")
        .short("w")
        .long("workdir")
        .value_name("DIR")
        .help("Sets the directory where Blockscape will store its data");
    
    /*let mut strpath: Option<&'a str> = None;

    let mut strpath = env::get_storage_dir().map(|p| String::from(p.to_str().unwrap()));

    if let Some(p) = strpath {
        workdir_arg = workdir_arg.default_value(&p);
    }*/

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
            merkle_root: U256_ZERO
        },
        txns: BTreeSet::new()
    };

    let mut m = Mutation::new();

    m.changes.push(Change::SetValue {
        key: Vec::from(ADMIN_KEY_PREFIX),
        // TODO: Put real admin key here
        value: Some(Vec::from(ADMIN_KEY)),
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

/// Converts the command line arguments to a client config ready to go
/// # Arguments
/// * `cmdline`: The argument matches from clap on the command line
/// *Note*: As this is a high level function, it will automatically try to load the network key from
/// file, and it will generate a new one if needed
/// # Panics
/// If it cannot save a newly created public key, or if the private key loaded is invalid
pub fn make_network_config(cmdline: &ArgMatches) -> ClientConfig {

    let key: PKey;

    if let Some(mut k) = load_key("node") {
        key = k;
        info!("Loaded node keyfile from file.");
    }
    else {
        info!("Generate node keyfile...");
        // need to create key
        key = generate_private_key();

        // save the key (fail if not saved)
        if !save_key("node", &key) {
            panic!("Could not save node private key file.");
        }
    }

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