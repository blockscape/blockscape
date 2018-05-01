use clap::{Arg, ArgGroup, ArgMatches, App};
use openssl::pkey::PKey;
use std::collections::BTreeSet;
use std::str::FromStr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::rc::Rc;

use serde_json;

use bincode;

use blockscape_core::bin::{Bin, AsBin};
use blockscape_core::env::*;
use blockscape_core::network::client::ClientConfig;
use blockscape_core::network::node::NodeEndpoint;
use blockscape_core::primitives::*;
use blockscape_core::forging::epos::EPoS;
use blockscape_core::signer::generate_private_key;
use blockscape_core::hash::hash_pub_key;
use blockscape_core::time::Time;
use blockscape_core::record_keeper::key::NetworkEntry;
use blockscape_core::rpc::RPC;
use blockscape_core::record_keeper::{RecordKeeperConfig, RecordKeeperIndexingStrategy};

use rpc;
use rules;
use game;

use context::Context;

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

/// TEMP testing key used for signing blocks into the network; right now everyone has the same one until we have a dedicated validator key infrastructure
/// TODO: Remove me
pub const TESTING_PRIVATE: &[u8] =
b"-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEAqsph1xGRJ2d2PxyVe9do0UNe4lcVEPi9Alb+kr+wZcbxU3uY
Kpny35eGzVVR8MJIIye5hk0lZqW0Mg6ksUa3upflGPeDzPS8Jd9PzU1XTbI1fGvH
4ExNID/hJaPzzjZ3HnfNOL1zGLecoaJHSTbXKvCw/bQQWidstGbTwc+OUwnFWveA
tvaDoTxCQVXM+Wg+7/aeASQgy5Hc/OPvsLXf8TdUayPyjNzKAbQNhf9kPnkmcokZ
FT1k+30l9g5Ff2xMHEx1rr3DfSMlHch3iG2pdCQqgE9JFo6R4Dh09kkK1ak0Crwx
LvPa22579+HQZuVD0AnDHx+jJW5JVs771z25qwIDAQABAoIBAHEn540T8YUW8mw8
JvpHLQZAybPSmH2HH8tWEhLueBPmrGtwXtAS8ayce0698a0/O4Y3Qp8tq9MHhI0J
0Ko3vXEeREa3bxazK5k4DGpsjKRIp1FJI8ODKjJswGIs71K4GVIRc+Hc+03sERWy
K+LhN8wWbl2ZGKBysH4SBsjJGHYA1SSihL2fpH3mNli4WJSTos43PMHdV7EEvxZF
Akm0tfhbD4JMYb+YzH0UCNPfDdf0sCl2E6bak+TGzpADu8OV4+TLL1GMwLlTkGsw
SL5QzGdsZe+s2/0535inLqBcuQEH3OdXW8Kx8K2Uyk7IF0nw+Al29wKw2+NySsNz
UhNCuJkCgYEA1T99KQ7bbaPEzp0XnuQ9UVTWFJ5/fzKv1klkbDQS+fdVMHizTjOq
top2jSk5c6tOaNQ0jT1vsbO2jAJDDgvX59TeHdP1bEHPjEKRPMagxML90ymSx+DP
rOaVPAjZjC/1twNO8bl2eK85H+domS/D/vJmhLt/F3FB96XurCE3weUCgYEAzQfa
V0dLiuHaQ2cfkUL+g1p2S+7kFYYDjgVKG0Nh71CKBGiRdSEeJdl47mhLUkF+5Wn8
ewXUlQhzfiAXxL3Zw6yjI5ugcUddL2Nwazj65Mv7iSnaZHAn5KXUohTFH2WRWjCS
XfUZdzw074+sGgNwtWtUoHSYjbThKiA/KMonFE8CgYEAx8lroYPh4J6GTGyxLJP5
PrGUwEyedrUuOD0acKV5AefPUFJE6wdM8ShYWXg98ziThXMKqSjd9EbCx/l2iTpf
VTwBvUBPttURdf8Hw0D0bmOhGqzgb5MX/o0pU82Ww9hLBON8mst/SyIfCtzrClnN
7pV7pu9i6ruZakNzkKCudGECgYEAq2mAUm2pq3/tIWLq0mAnNpv/wLYFfDUhba/g
Z/CqxRAZg1wFF97LPKuXXgJVznwxYg1850FVnA+Htw+Pr41lrSD89z0aIvqd3ouN
JidqIrSjI+aYzlWyFIfLwIIK15frsHJhPCo40yXDv/Dm2oy7wwDrrIYuMHLjuHtj
Mm/nwiMCgYAXyLfA3yQMIom5kv7fsA20UiEDeA+d5Gjdpwy5ILHDDjtDMt1LdkCm
JoOyAYZ1e7ZoN9ydJO9c5vPtrUelVaS1PCrb9c9s04980FfdCNMo9ZXyVQt9C1H4
OsByU0oHDhWmPAlcVQbgqLzOTKaWYn4mle2iqgC0pR8kDBWcQJ/55Q==
-----END RSA PRIVATE KEY-----";

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
            .arg(Arg::with_name("forge")
                .long("forge")
                .short("F")
                .help("Run the forging application on all primary shards"))
        .group(ArgGroup::with_name("blockchain"))
            .arg(Arg::with_name("mempool-size")
                .long("mempool-size")
                .help("The maximum amount of memory reserved for storing pending transactions (i.e. not accepted into a block)")
                .value_name("BYTES")
                .default_value("128M"))
            .arg(Arg::with_name("indexing")
                .long("indexing")
                .short("I")
                .help("The indexing strategy utilized by RecordKeeper, should be modified to your requirements (either 'full', 'standard', or 'light')")
                .default_value("standard"))
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
    let genesis_extra_blob = EPoS::genesis_block_data();

    let admkey: Bin = PKey::public_key_from_pem(ADMIN_KEY).unwrap()
        .public_key_to_der().unwrap()
        .into();

    let testkey: Bin = PKey::private_key_from_pem(TESTING_PRIVATE).unwrap()
        .public_key_to_der().unwrap()
        .into();

    let mut m = Mutation::new();

    let adm_key_hash = hash_pub_key(&admkey);

    m.changes.push(Change::Admin {
        key: NetworkEntry::AdminKeyID.as_bin(),
        value: Some(adm_key_hash.as_bin())
    });
    m.changes.push(Change::NewValidator{pub_key: admkey});
    m.changes.push(Change::NewValidator{pub_key: testkey});

    let txn = Txn {
        timestamp: Time::from_seconds(1508009036),
        creator: adm_key_hash,
        mutation: m,
        signature: Bin::new(),
    };

    let mut txns = BTreeSet::new();
    txns.insert(txn.calculate_hash());
    let merkle_root = Block::calculate_merkle_root(&txns);

    let b = Block {
        header: BlockHeader {
            version: 1,
            timestamp: Time::from_seconds(1508009036),
            shard: U256_ZERO,
            prev: U256_ZERO,
            merkle_root,
            // Serialize in the initial block difficulty
            blob: bincode::serialize(&genesis_extra_blob, bincode::Infinite).unwrap()
        },
        txns
    };

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

fn decode_bytes(s: &str) -> u64 {
    // read the last character, which indicates the representation
    match s.to_uppercase().chars().last().unwrap() {
        'K' => s[..s.len() - 1].parse::<u64>().map(|v| v * 1024u64),
        'M' => s[..s.len() - 1].parse::<u64>().map(|v| v * 1024u64.pow(2)),
        'G' => s[..s.len() - 1].parse::<u64>().map(|v| v * 1024u64.pow(3)),
        'T' => s[..s.len() - 1].parse::<u64>().map(|v| v * 1024u64.pow(4)),
        _ => s.parse::<u64>()
    }.expect("Bytes must be a number, or end in the appropriate suffix (K, M, G, T)")
}

/// Generates the record keeper configuration from checkers game rules and the command line arguments
pub fn make_rk_config(cmdline: &ArgMatches, cache: &game::GameCache) -> RecordKeeperConfig {

    let strategy = match cmdline.value_of_lossy("indexing").unwrap().as_ref() {
        "full" => RecordKeeperIndexingStrategy::Full,
        "standard" => RecordKeeperIndexingStrategy::Standard,
        "light" => RecordKeeperIndexingStrategy::Light,
        _ => panic!("Invalid indexing strategy (expected 'full', 'standard', or 'light'")
    };

    RecordKeeperConfig {
        pending_txn_limit: decode_bytes(&cmdline.value_of_lossy("mempool-size").unwrap()),
        index_strategy: strategy,
        rules: rules::build_rules(Arc::clone(cache)),
    }
}

/// Starts the JSONRPC server
pub fn make_rpc(cmdline: &ArgMatches, ctx: Rc<Context>) -> RPC {

    let bind_addr = SocketAddr::new(cmdline.value_of("rpcbind").unwrap().parse().expect("Invalid RPC bind IP"), 
            cmdline.value_of("rpcport").unwrap().parse::<u16>().expect("Invalid RPC port: must be a number!"));

    rpc::make_rpc(&ctx, bind_addr)
}

pub fn call_rpc(cmdline: &ArgMatches) -> i32 {

    use blockscape_core::rpc::client::JsonRpcRequest;

    let method = cmdline.value_of_lossy("rpccmd").expect("Unknown encoding for RPC command!").into_owned();
    let raw_args = cmdline.values_of_lossy("rpcargs").unwrap_or(Vec::new());

    let a = if raw_args.len() == 1 {
        let res: Result<serde_json::Value, _> = serde_json::from_str(&raw_args[0]);
        if let Ok(r) = res {
            serde_json::to_value([r]).unwrap()
        }
        else {
            //println!("Could not parse JSON arguments: {:?}", res);
            //println!("Trying to send literal string...");

            serde_json::to_value(raw_args).unwrap()
        }
    }
    else {
        serde_json::to_value(raw_args).unwrap()
    };

    debug!("Calling RPC: {}", method);

    let bind_addr = SocketAddr::new(cmdline.value_of("rpcbind").unwrap().parse().expect("Invalid RPC bind IP"), 
            cmdline.value_of("rpcport").unwrap().parse::<u16>().expect("Invalid RPC port: must be a number!"));

    let res = JsonRpcRequest::new(method, a).exec_sync(bind_addr);

    if res.is_err() {
        println!("RPC Error: {}", res.err().unwrap());

        1
    }
    else {
        let r = res.unwrap();
        println!("{}", r);

        r.get_exit_code() as i32
    }
}