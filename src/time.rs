use timelib::{Timespec, strftime, at_utc};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents an instant in time, defined by the number of milliseconds since the UNIX Epoch
#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
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

impl From<Time> for i64 {
    fn from(t: Time) -> i64 {
        t.0
    }
}

impl From<i64> for Time {
    fn from(t: i64) -> Time {
        Time(t)
    }
}

impl Time {
    pub fn from_milliseconds(ms: i64) -> Time {
        Time(ms)
    }

    pub fn from_seconds(s: i64) -> Time {
        Time::from_milliseconds(s * 1000i64)
    }

    /// Return the current time in ms since the epoch. Later this can be switched to use NTP.
    pub fn current() -> Time {
        let duration_since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seconds_contrib = (duration_since_epoch.as_secs() as i64) * 1_000i64;
        let nseconds_contrib = (duration_since_epoch.subsec_nanos() as i64) / 1_000_000i64;
        let milliseconds = seconds_contrib + nseconds_contrib;
        Time(milliseconds)
    }
}



#[test]
fn debug_print() {
    let t = Time(1505679102000);

    assert_eq!(format!("{:?}", t), "09-17-17 20:11:42");
}

#[test]
fn current() {
    let t = Time::current();
    let time_of_writing = Time::from_seconds(1506487146i64);
    assert!(t > time_of_writing);
}