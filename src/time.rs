use timelib::at;
use timelib::Tm;
use timelib::Timespec;

use std::fmt;

/// Represents an instant in time, defined by the number of milliseconds since the UNIX Epoch
#[derive(Serialize, Deserialize)]
pub struct Time(i64);

impl fmt::Debug for Time {
    /// Write the time as a formatted date
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // write a formatted date
        let ts = Timespec {
            sec: &self.0 / 1000,
            nsec: (&self.0 % 1000) as i32 * 1000000,
        };
        let t = at(ts);
        write!(f, "{:?}", t)
    }
}

// TODO: Implement test now!
