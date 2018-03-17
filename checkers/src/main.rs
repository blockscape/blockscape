extern crate openssl;
extern crate serde;
extern crate serde_json;

extern crate futures;
extern crate tokio_core;
extern crate tokio_signal;
extern crate parking_lot;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate clap;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

extern crate blockscape_core;

extern crate colored;

extern crate bincode;

mod boot;
mod context;
mod rules;
mod reporter;
mod format;
mod forger;
mod game;

mod rpc;

mod checkers;

use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::rc::Rc;

use futures::prelude::*;
use futures::sync::oneshot::channel;

use tokio_core::reactor::*;

use openssl::pkey::PKey;

use blockscape_core::env;
use blockscape_core::forging::epos::{EPoS, EPoSConfig};
use blockscape_core::network::client::*;
use blockscape_core::record_keeper::{RecordKeeper};

use game::CheckersGame;

use boot::*;

use context::Context;

fn main() {
    pretty_env_logger::init().unwrap();

    let mut core = Core::new().expect("Could not start main event loop");

    // Parse cmdline
    let cmdline = parse_cmdline();

    // are we to be executing an RPC command on a running instance?
    if cmdline.is_present("rpccmd") {
        std::process::exit(call_rpc(&cmdline));
    }

    // Ready to boot
    println!("Welcome to Blockscape v{}", env!("CARGO_PKG_VERSION"));
    debug!("Debug logging ENABLED.");

    // Open database; populate basic subsystems with latest information
    if let Some(d) = cmdline.value_of("workdir") { 
        env::prepare_storage_dir(&String::from(d).into());
    }
    else {
        env::prepare_storage_dir(&env::get_storage_dir()
            .expect("Could not automatically find work directory for blockscape! Please check your environment and try again."));
    }

    // TODO: Somewhere around here, we read a config or cmdline or something to figure out which net to work for
    // but start with the genesis
    let genesis = make_genesis();
    let genesis_net = genesis.0.calculate_hash();

    let game_cache = game::create_cache();
    let rk = Arc::new(
        RecordKeeper::open(
            {let mut p = env::get_storage_dir().unwrap(); p.push("db"); p},
            make_rk_config(&cmdline, &game_cache),
            genesis
        ).expect("Record Keeper was not able to initialize!")
    );

    let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();
    let (qs, qr) = channel::<()>();

    let quit = Box::new(qr).shared();

    // start network
    let cc = make_network_config(&cmdline);
    let (h, t) = Client::run(cc, Arc::clone(&rk), quit.clone()).expect("Could not start network");

    // must be connected to at least one network in order to do anything, might as well be genesis for now.
    let net_client = h.send(ClientMsg::AttachNetwork(genesis_net, ShardMode::Primary)).wait().expect("Could not attach to root network!");
    threads.push(t);

    let checkers_game = Arc::new(CheckersGame{ 
        rk: Arc::clone(&rk), 
        sign_key: PKey::private_key_from_pem(boot::TESTING_PRIVATE).unwrap(), 
        cache: game_cache 
    });

    let forge_key = PKey::private_key_from_pem(boot::TESTING_PRIVATE).unwrap();

    let ctx = Rc::new(Context {
        rk: rk.clone(),
        network: net_client.clone(),
        game: checkers_game,
        // this block forger will be callibrated to mine a block every 10 seconds, with 6 hours before each recalculate
        forge_algo: EPoS::new(rk, net_client, core.handle().remote().clone(), EPoSConfig {
                rate_target: 12 * 1000, // 12 seconds
                recalculate_blocks: 1800, // 6 hours 
                validators_scan: 100,
                validators_count_base: 3,
                signing_keys: vec![forge_key.private_key_to_der().unwrap()]
            }).expect("Could not start the proof of stake forger!"),

        forge_key: forge_key
    });

    // Open RPC interface
    let rpc = make_rpc(&cmdline, Rc::clone(&ctx));

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

    // let context = Rc::clone(&ctx);
    // let test_txn_job = Interval::new(Duration::from_millis(7500), &handler)
    //     .unwrap()
    //     .for_each(move |_| {
    //         let mut mutation = Mutation::new();
    //         mutation.changes.push(Change::Event{id: PlotID(0, 0), tick: Time::current().millis() as u64, event: PlotEvent { from: PlotID(0, 0), to: PlotID(0, 0), event: Bin::new()}});
    //         context.rk.add_pending_txn(&Txn::new(hash_pub_key(&context.forge_key.public_key_to_der().unwrap()), mutation).sign(&context.forge_key)).unwrap();
    //         Ok(())
    //     })
    //     .map_err(|_| ());

    core.handle().spawn(rpt_job);
    // core.handle().spawn(test_txn_job);

    if cmdline.is_present("forge") {
        forger::start_forging(&ctx, &handler, genesis_net);
    }

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