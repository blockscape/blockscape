
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