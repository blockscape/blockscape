use timelib::at_utc;
use timelib::strftime;
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

        write!(f, "{}", strftime("%m-%d-%y %H:%M:%S", &at_utc(ts)).unwrap())
    }
}



#[test]
fn debug_print() {
    let t = Time(1505679102000);

    assert_eq!(format!("{:?}", t), "09-17-17 20:11:42");
}