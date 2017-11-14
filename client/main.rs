extern crate blockscape_core;
extern crate chan_signal;
extern crate openssl;
extern crate serde;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate clap;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

extern crate colored;

mod boot;
mod plot_event;
mod rules;
mod reporter;
mod format;

use chan_signal::Signal;
use clap::{Arg, ArgGroup, ArgMatches, App, SubCommand};
use std::sync::Arc;
use std::thread;
use std::sync::mpsc::channel;

use blockscape_core::env;
use blockscape_core::network::client::{Client, ShardMode};
use blockscape_core::primitives::HasBlockHeader;
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::work_queue::WorkQueue;
use plot_event::PlotEvent;

use boot::*;

fn main() {
    pretty_env_logger::init().unwrap();

    // Parse cmdline
    let cmdline = parse_cmdline();

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

    let db = Arc::new(RecordKeeper::open(None, Some(rules::build_rules())).expect("Record Keeper was not able to initialize!"));
    let wq = Arc::new(WorkQueue::new(db.clone()));


    let mut net_client: Option<Arc<Client>> = None;

    let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();

    if !cmdline.is_present("disable-net") {
        // start network
        let cc = make_network_config(&cmdline);

        let mut c = Client::new(db, wq, cc);
        // should be okay because we are still on a single thread at this point, so open the client
        Arc::get_mut(&mut c).expect("Could not mutably aquire client to open it!").open();

        // TODO: Somewhere around here, we read a config or cmdline or something to figure out which net to work for
        // but start with the genesis
        let genesis_net = make_genesis().0.get_header().calculate_hash();

        // must be connected to at least one network in order to do anything, might as well be genesis for now.
        c.attach_network(genesis_net, ShardMode::Primary);

        net_client = Some(c);

        // start networking threads and handlers
        let mut ts = Client::run(net_client.clone().unwrap());

        while let Some(t) = ts.pop() {
            threads.push(t);
        }
    }

    // Open RPC interface


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