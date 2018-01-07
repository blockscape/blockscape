extern crate libc;
extern crate openssl;
extern crate serde;
extern crate serde_json;

extern crate futures;
extern crate hyper;
extern crate tokio_core;
extern crate tokio_signal;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate clap;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

extern crate blockscape_core;

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

use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::rc::Rc;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::sync::oneshot::channel;

use tokio_core::reactor::*;

use blockscape_core::env;
use blockscape_core::network::client::*;
use blockscape_core::primitives::HasBlockHeader;
use blockscape_core::record_keeper::RecordKeeper;

use boot::*;

use context::Context;

fn main() {
    pretty_env_logger::init().unwrap();

    // Parse cmdline
    let cmdline = parse_cmdline();

    // are we to be executing an RPC command on a running instance?
    if cmdline.is_present("rpccmd") {
        if call_rpc(&cmdline) {
            return;
        }
        else {
            println!("Exiting with Failure");

            std::process::exit(1);
        }
    }

    // Ready to boot
    println!("Welcome to Blockscape v{}", env!("CARGO_PKG_VERSION"));

    // Open database; populate basic subsystems with latest information
    if let Some(d) = cmdline.value_of("workdir") { 
        env::prepare_storage_dir(&String::from(d).into());
    }
    else {
        env::prepare_storage_dir(&env::get_storage_dir()
            .expect("Could not automatically find work directory for blockscape! Please check your environment and try again."));
    }

    let rk = Arc::new(RecordKeeper::open(load_or_generate_key("user"), None, Some(rules::build_rules())).expect("Record Keeper was not able to initialize!"));

    let mut net_client: Option<UnboundedSender<ClientMsg>> = None;

    let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();
    let (qs, qr) = channel::<()>();

    let quit = Box::new(qr).shared();

    if !cmdline.is_present("disable-net") {
        // start network
        let cc = make_network_config(&cmdline);

        let (mut h, t) = Client::run(cc, rk.clone(), quit.clone()).expect("Could not start network");

        // TODO: Somewhere around here, we read a config or cmdline or something to figure out which net to work for
        // but start with the genesis
        let genesis_net = make_genesis().0.get_header().calculate_hash();

        // must be connected to at least one network in order to do anything, might as well be genesis for now.
        h = h.send(ClientMsg::AttachNetwork(genesis_net, ShardMode::Primary)).wait().expect("Could not attach to root network!");

        net_client = Some(h);
        threads.push(t);
    }

    let ctx = Rc::new(Context {
        rk: rk.clone(),
        network: net_client.clone()
    });

    // Open RPC interface
    let rpc = make_rpc(&cmdline, Rc::clone(&ctx));

    let mut core = Core::new().expect("Could not start main event loop");

    let handler = core.handle();

    let context = Rc::clone(&ctx);
    let h2 = core.handle();
    let rpt_job = Interval::new(Duration::from_secs(30), &handler)
        .unwrap()
        .for_each(move |_| {
            reporter::do_report(&context, &h2);
            Ok(())
        })
        .map_err(|_| ());

    core.handle().spawn(rpt_job);

    // wait for the kill signal
    let qsignal = tokio_signal::ctrl_c(&handler).flatten_stream().take(1);

    core.run(qsignal.into_future()).ok().unwrap();

    println!("Finishing work, please wait...");

    rpc.close();
    qs.send(()).expect("Could not send quit signal to handlers.");

    debug!("Waiting for threads...");
    
    while let Some(thread) = threads.pop() {
        thread.join().expect("Failed to join thread");
    }

    println!("Exiting...");
}