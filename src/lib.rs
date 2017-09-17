extern crate bincode;
extern crate bytes;
extern crate crypto;
extern crate serde_json;
extern crate serde;
extern crate time as timelib;

#[macro_use]
extern crate serde_derive;

mod block;
mod transaction;
mod network;
mod test;
mod u256;
mod time;

pub fn do_stuff(x: i32) -> i32 {
    x * 5
}
