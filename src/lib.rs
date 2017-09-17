extern crate bincode;
extern crate bytes;
extern crate crypto;
extern crate serde_json;
extern crate serde;

#[macro_use]
extern crate serde_derive;


#[cfg(test)]
mod block;
mod test;
mod u256;


pub fn do_stuff(x: i32) -> i32 {
    x * 5
}
