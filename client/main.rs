#[macro_use]
extern crate log;
extern crate env_logger;

extern crate blockscape_core;

use blockscape_core::env;
use blockscape_core::network::client::Client;

fn main() {
    env_logger::init().unwrap();

    // TODO: Parse a ton of cmdlines

    println!("Welcome to Blockscape v{}", env!("CARGO_PKG_VERSION"));

    // Open database; populate basic subsystems with latest information
    env::prepare_storage_dir();

    // Open network, start peer protocol
    //let client = Client::new();

    // Open RPC interface

    println!("Exiting...");
}