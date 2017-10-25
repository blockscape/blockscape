use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;

use std::time::Duration;

use colored::*;

use blockscape_core::network::client::Client;
use blockscape_core::time::Time;

const PRINT_FREQUENCY: i64 = 30 * 1000; // print statistics every 30 seconds

pub fn run(client: &Option<Arc<Client>>, rx: Receiver<()>) {

    let mut last_print = Time::current();

    loop {
        //thread::sleep(::std::time::Duration::from_millis(1000));
        if rx.recv_timeout(Duration::from_millis(1000)).is_ok() {
            return; // right now if any message is sent, it means quit
        }

        let n = Time::current();

        if last_print.diff(&n).millis() > PRINT_FREQUENCY {
            
            // print out stuff
            // network stats
            if let Some(ref c) = *client {
                let net_stats = c.get_stats();
                let config = c.get_config();

                println!("{}\t{} nets\t{}/{} peers\t{} in\t{} out", 
                    "NET:".bold(),
                    value_print(net_stats.attached_networks, 0, 3),
                    value_print(net_stats.connected_peers, config.min_nodes as u32 * net_stats.attached_networks as u32, config.max_nodes as u32 * net_stats.attached_networks as u32),
                    config.max_nodes.to_string(),
                    net_stats.rx.to_string().yellow(),
                    net_stats.tx.to_string().yellow()
                );
            }

            last_print = n;
        }
    }
}

/// Returns a colored representation of the value, coloring based on the given thresholds.
/// Prints red if it is less than or equal to `low`
/// Prints blue if it is less than `high` and greater than `low`
/// Prints green otherwise
fn value_print<N: PartialOrd + ToString>(val: N, low: N, high: N) -> ColoredString {
    if val <= low {
        val.to_string().red()
    }
    else if val < high {
        val.to_string().cyan()
    }
    else {
        val.to_string().green()
    }
}

/// Same as `value_print`, but prints the "oppoite" colors when high values are the extreme/bad
fn inverse_value_print<N: PartialOrd + ToString>(val: N, low: N, high: N) -> ColoredString {
    if val <= low {
        val.to_string().green()
    }
    else if val < high {
        val.to_string().cyan()
    }
    else {
        val.to_string().red()
    }
}