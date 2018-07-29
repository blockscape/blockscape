use bin::{AsBin, Bin};

use futures::future::Shared;
use futures::sync::oneshot::Receiver;

pub type QuitSignal = Shared<Box<Receiver<()>>>;

/// For use in initializing an array when Copy is not implementable on a type
/// See: https://www.reddit.com/r/rust/comments/33xhhu/how_to_create_an_array_of_structs_that_havent/cqqf9tr/
macro_rules! init_array(
    ($ty:ty, $len:expr, $val:expr) => (
        {
            let mut array: [$ty; $len] = unsafe { ::std::mem::uninitialized() };
            for i in array.iter_mut() {
                unsafe { ::std::ptr::write(i, $val); }
            }

            array
        }
    )
);

/// Like `try!`, but with futures!
macro_rules! tryf(
    ($e:expr) => {
        match $e {
            Ok(k) => k,
            Err(e) => return Box::new(future::err(e))
        }
    }
);

/// Verifies if the given binary string matches a simple pattern. Only supports asterisk for right now,
/// and asterisk cannot be used in bin at all.
fn simple_match(topic: &Bin, pattern: &Bin) -> bool {
    assert!(!topic.contains(&b'*'), "Simple match string contained wildcard pattern");

    if pattern.is_empty() {
        return topic.is_empty();
    }

    let p = pattern.split(|c| *c == b'*');

    // topic must contain all items of p as a "substring" in the same order.
    // Also first substring must be at the beginning, the last at the end.
    let mut tp = 0;

    for x in p {
        if x.is_empty() {
            continue;
        }

        while tp < topic.len() {
            let mut xp = 0;
            while tp <= topic.len() - x.len() && topic[tp] != x[xp] {
                tp += 1;
            }

            if tp > topic.len() - x.len() {
                return false; // ran out of room
            }

            xp += 1;
            while xp < x.len() {
                
                if topic[tp + xp] != x[xp] {
                    tp += 1;
                    break;
                }

                xp += 1;
            }

            if xp == x.len() {
                tp += xp;
                break; // found a match
            }
        }
    }

    tp >= topic.len() || *pattern.last().unwrap() == b'*' as u8
}

#[test]
fn simple_match_test() {
    assert!(simple_match(&b"hello".to_vec(), &b"hello".to_vec()));
    assert!(simple_match(&b"hello".to_vec(), &b"*".to_vec()));
    assert!(simple_match(&b"hello".to_vec(), &b"h*llo".to_vec()));
    assert!(simple_match(&b"hello".to_vec(), &b"hell*".to_vec()));
    assert!(simple_match(&b"hello".to_vec(), &b"*ello".to_vec()));
    assert!(simple_match(&b"hello".to_vec(), &b"*ll*".to_vec()));
    assert!(!simple_match(&b"hello".to_vec(), &b"*le*".to_vec()));
    assert!(!simple_match(&b"hello".to_vec(), &b"".to_vec()));
    assert!(simple_match(&b"".to_vec(), &b"".to_vec()));
}