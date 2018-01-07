/*use ntp::request;

use timelib::{Timespec, now_utc};

pub fn calc_drift(ntp_server: &str) -> Result<i64, String> {
    debug!("NTP request server: {}", ntp_server);
    request(ntp_server)
        .map(|p| {
            let dest_time = now_utc().to_timespec();
            let orig_time = Timespec::from(p.orig_time);
            let recv_time = Timespec::from(p.recv_time);
            let transmit_time = Timespec::from(p.transmit_time);

            (((recv_time - orig_time) + (transmit_time - dest_time)) / 2).num_milliseconds()
        })
        .map_err(|e| String::from(e.description()))
}*/