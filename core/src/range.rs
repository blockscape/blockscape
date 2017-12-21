use std::ops::Add;

/// A simple Python-esque range Object in the form Range(start, stop, step).
/// https://stackoverflow.com/questions/27893223/how-do-i-iterate-over-a-range-with-a-custom-step
pub struct Range<T>(pub T, pub T, pub T)
    where for<'a> &'a T: Add<&'a T, Output = T>,
          T: PartialOrd,
          T: Clone;

impl<T> Iterator for Range<T>
    where for<'a> &'a T: Add<&'a T, Output = T>,
          T: PartialOrd,
          T: Clone
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        if self.0 < self.1 {
            let v = self.0.clone();
            self.0 = &v + &self.2;
            Some(v)
        } else {
            None
        }
    }
}