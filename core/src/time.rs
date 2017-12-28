use timelib::{Timespec, strftime, at_utc};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use std::sync::atomic::{AtomicIsize,ATOMIC_ISIZE_INIT};
use std::sync::atomic::Ordering::Relaxed;

/// Represents an instant in time, defined by the number of milliseconds since the UNIX Epoch
#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord, Copy, Clone, Hash)]
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

impl Into<i64> for Time {
    fn into(self) -> i64 {
        self.0
    }
}

impl From<i64> for Time {
    fn from(t: i64) -> Time {
        Time(t)
    }
}


static NTP_DRIFT: AtomicIsize = ATOMIC_ISIZE_INIT;

impl Time {

    /// The time drift correction as calculated by NTP, in milliseconds
    /// Updated by the network thread automatically
    pub fn update_ntp(drift: i64) {
        if NTP_DRIFT.load(Relaxed) == 0 {
            // just set to start
            NTP_DRIFT.store(drift as isize, Relaxed);
        }
        else {
            // weighted
            NTP_DRIFT.store((drift as f64 * 0.1 + NTP_DRIFT.load(Relaxed) as f64 * 0.9 as f64) as isize, Relaxed);
        }
    }

    pub fn from_milliseconds(ms: i64) -> Time {
        Time(ms)
    }

    pub fn from_seconds(s: i64) -> Time {
        Time::from_milliseconds(s * 1000i64)
    }

    /// Return the current time in ms since the epoch. This includes a drift adjustment for NTP
    pub fn current() -> Time {
        let duration_since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seconds_contrib = (duration_since_epoch.as_secs() as i64) * 1_000i64;
        let nseconds_contrib = (duration_since_epoch.subsec_nanos() as i64) / 1_000_000i64;
        let milliseconds = seconds_contrib + nseconds_contrib;
        // correct for drift
        Time(milliseconds - NTP_DRIFT.load(Relaxed) as i64)
    }

    /// Return the current time in ms since the epoch. This is ***without*** a drift adjustment from NTP
    pub fn current_local() -> Time {
        let duration_since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seconds_contrib = (duration_since_epoch.as_secs() as i64) * 1_000i64;
        let nseconds_contrib = (duration_since_epoch.subsec_nanos() as i64) / 1_000_000i64;
        let milliseconds = seconds_contrib + nseconds_contrib;
        // correct for drift
        Time(milliseconds)
    }

    /// Return the time in milliseconds as a simple integer representation.
    pub fn millis(&self) -> i64 {
        self.0
    }

    /// Modify this time as a weighted average
    pub fn apply_weight(&mut self, weight: &Time, factor: f32) {
        let f = 1.0 / factor;
        self.0 = (weight.0 as f64 * f as f64 + self.0 as f64 * (1.0 - f) as f64) as i64;
    }

    pub fn diff(&self, other: &Time) -> Time {
        Time::from_milliseconds(other.0 - self.0)
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