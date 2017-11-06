const BYTE_FACTOR: u64 = 1024;

pub fn as_bytes(val: u64) -> String {
    if val < BYTE_FACTOR {
        val.to_string() + "b"
    }
    else if val < BYTE_FACTOR * BYTE_FACTOR {
        format!("{:.2}", val as f64 / BYTE_FACTOR as f64) + "KB"
    }
    else if val < BYTE_FACTOR * BYTE_FACTOR * BYTE_FACTOR {
        format!("{:.2}", val as f64 / (BYTE_FACTOR * BYTE_FACTOR) as f64) + "MB"
    }
    else {
        format!("{:.2}", val as f64 / (BYTE_FACTOR * BYTE_FACTOR * BYTE_FACTOR) as f64) + "GB"
    }
}

pub fn as_bytes_per_sec(val: u64) -> String {
    as_bytes(val) + "/s"
}

pub fn as_secs(val: u64) -> String {
    if val < 1000 {
        val.to_string() + "ms"
    }
    else if val < 300 * 1000 { // 5 minutes
        format!("{:.2}", val as f64 / 1000.0) + "s"
    }
    else if val < 60 * 120 * 1000 { // 2 hours
        format!("{:.2}", val as f64 / (60.0 * 1000.0)) + "m"
    }
    else {
        format!("{:.2}", val as f64 / (60.0 * 60.0 * 1000.0)) + "h"
    }
}