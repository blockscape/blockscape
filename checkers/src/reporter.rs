use std::rc::Rc;

use colored::*;

use format::*;

use context::Context;

use tokio_core::reactor::Handle;

use futures::prelude::*;
use futures::sync::oneshot;

use blockscape_core::network::client::*;

pub fn do_report(context: &Rc<Context>, handler: &Handle) {
    let (tx, rx) = oneshot::channel();
    if let Err(e) = context.network.unbounded_send(ClientMsg::GetStatistics(tx)) {
        println!("ERROR: Could not collect network stats: {}", e);
        return;
    }


    let f = rx.and_then(|net_stats| {
        println!("{}\t{} nets\t{} peers\t{} in\t{} out", 
            "NET:".bold(),
            value_print(net_stats.attached_networks, 0, 3),
            value_print(net_stats.connected_peers, 8 * net_stats.attached_networks as u32, 16 * net_stats.attached_networks as u32),
            as_bytes(net_stats.rx).yellow(),
            as_bytes(net_stats.tx).yellow()
        );

        Ok(())
    }).or_else(|_| {
        println!("ERROR: Could not collect network stats: cancelled");

        Err(())  
    });

    handler.spawn(f);
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

/*
// Same as `value_print`, but prints the "oppoite" colors when high values are the extreme/bad
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

fn value_string_print<N: PartialOrd>(val: N, low: N, high: N, s: &str) -> ColoredString {
    if val <= low {
        s.red()
    }
    else if val < high {
        s.cyan()
    }
    else {
        s.green()
    }
}

fn inverse_value_string_print<N: PartialOrd>(val: N, low: N, high: N, s: &str) -> ColoredString {
    if val <= low {
        s.green()
    }
    else if val < high {
        s.cyan()
    }
    else {
        s.red()
    }
}*/