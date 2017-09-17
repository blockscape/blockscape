extern crate u256;
extern crate crypto;
extern crate bytes;


#[cfg(test)]
mod test;
mod block;
mod network;

pub fn do_stuff(x: i32) -> i32 {
    x * 5
}
