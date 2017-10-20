#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate env_logger;

extern crate openssl;

extern crate blockscape_core;

extern crate chan_signal;

mod boot;
mod rules;

use std::sync::Arc;
use std::thread::JoinHandle;

use clap::{Arg, ArgGroup, ArgMatches, App, SubCommand};

use chan_signal::Signal;

use blockscape_core::env;
use blockscape_core::network::client::Client;
use blockscape_core::record_keeper::database::Database;

use boot::*;

fn main() {
    env_logger::init().unwrap();

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

    let db = Arc::new(Database::open_db(Some(rules::build_rules())).expect("Database was not able to initialize!"));


    let mut net_client: Option<Arc<Client>> = None;

    let mut threads: Vec<JoinHandle<()>> = Vec::new();

    if !cmdline.is_present("disable-net") {
        // start network
        let cc = make_network_config(&cmdline);

        let mut c = Client::new(db, cc);
        c.open();

        net_client = Some(
            Arc::new(c)
        );

        // start networking threads and handlers
        let mut ts = Client::run(net_client.clone().unwrap());

        while let Some(t) = ts.pop() {
            threads.push(t);
        }
    }

    // Open RPC interface

    // wait for the kill signal
    signal.recv().unwrap();

    println!("Finishing work, please wait...");

    // close the network
    if let Some(client) = net_client {
        client.close();
    }
    
    threads.pop().unwrap().join();

    println!("Exiting...");
}