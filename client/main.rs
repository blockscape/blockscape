extern crate blockscape_core;
extern crate chan_signal;
extern crate openssl;
extern crate serde;
extern crate serde_json;

extern crate futures;
extern crate hyper;
extern crate tokio_core;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate clap;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

extern crate colored;

extern crate jsonrpc_core;
extern crate jsonrpc_macros;
extern crate jsonrpc_http_server;

mod boot;
mod context;
mod plot_event;
mod rules;
mod reporter;
mod format;

mod rpc;

use chan_signal::Signal;
use std::sync::Arc;
use std::thread;
use std::sync::mpsc::channel;

use blockscape_core::env;
use blockscape_core::network::client::{Client, ShardMode};
use blockscape_core::primitives::HasBlockHeader;
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::work_queue::WorkQueue;

use boot::*;

use context::Context;

fn main() {
    pretty_env_logger::init().unwrap();

    // Parse cmdline
    let cmdline = parse_cmdline();

    // are we to be executing an RPC command on a running instance?
    if cmdline.is_present("rpccmd") {
        call_rpc(&cmdline);
        return;
    }

    // Ready to boot
    println!("Welcome to Blockscape v{}", env!("CARGO_PKG_VERSION"));

    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);

    // Open database; populate basic subsystems with latest information
    if let Some(d) = cmdline.value_of("workdir") { 
        env::prepare_storage_dir(&String::from(d).into());
    }
    else {
        env::prepare_storage_dir(&env::get_storage_dir()
            .expect("Could not automatically find work directory for blockscape! Please check your environment and try again."));
    }

    let rk = Arc::new(RecordKeeper::open(None, Some(rules::build_rules())).expect("Record Keeper was not able to initialize!"));
    let wq = Arc::new(WorkQueue::new(rk.clone()));


    let mut net_client: Option<Arc<Client>> = None;

    let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();

    if !cmdline.is_present("disable-net") {
        // start network
        let cc = make_network_config(&cmdline);

        let c = Client::new(rk.clone(), wq, cc);

        // TODO: Somewhere around here, we read a config or cmdline or something to figure out which net to work for
        // but start with the genesis
        let genesis_net = make_genesis().0.get_header().calculate_hash();

        // must be connected to at least one network in order to do anything, might as well be genesis for now.
        c.attach_network(genesis_net, ShardMode::Primary).expect("Could not attach to a network!");

        net_client = Some(c);

        // start networking threads and handlers
        let mut ts = Client::run(net_client.clone().unwrap());

        while let Some(t) = ts.pop() {
            threads.push(t);
        }
    }

    let ctx = Context {
        rk: rk.clone(),
        network: net_client.clone()
    };

    // Open RPC interface
    let rpc = make_rpc(&cmdline, ctx.clone());

    // startup the reporter
    let (tx, rx) = channel();
    {
        let nc = net_client.clone();

        threads.push(
            thread::Builder::new().name(String::from("Reporter")).spawn(move || {
                reporter::run(&nc, rx);
            }).unwrap()
        );
    }

    // wait for the kill signal
    signal.recv().unwrap();

    println!("Finishing work, please wait...");

    rpc.close();

    // close the network
    if let Some(client) = net_client {
        client.close();
    }

    tx.send(()).expect("Thread was finished prematurely");

    debug!("Waiting for threads...");
    
    while let Some(thread) = threads.pop() {
        thread.join().expect("Failed to join thread");
    }

    println!("Exiting...");
}