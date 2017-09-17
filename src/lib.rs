extern crate bincode;
extern crate bytes;
extern crate crypto;
extern crate serde_json;
extern crate serde;
extern crate time as timelib;

#[macro_use]
extern crate serde_derive;

pub mod block;
pub mod transaction;
pub mod network;
mod test;
pub mod u256;
pub mod time;

pub fn do_stuff(x: i32) -> i32 {
    x * 5
}
