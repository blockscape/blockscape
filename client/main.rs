#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate env_logger;

extern crate openssl;

extern crate blockscape_core;

mod boot;

use clap::{Arg, ArgGroup, ArgMatches, App, SubCommand};

use blockscape_core::env;
use blockscape_core::network::client::Client;

use boot::*;

fn main() {
    env_logger::init().unwrap();

    // Parse cmdline
    let cmdline = parse_cmdline();

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

    // Open network, start peer protocol
    //let client = Client::new();

    // Open RPC interface

    println!("Exiting...");
}