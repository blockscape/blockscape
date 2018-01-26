use std::sync::Arc;
use std::rc::Rc;

use context::Context;

use tokio_core::reactor::Handle;

use futures::prelude::*;
use futures::future::{self, Either};
use futures::sync::mpsc;
use futures::sync::oneshot;
use futures;

use blockscape_core::record_keeper::RecordEvent;
use blockscape_core::primitives::*;
use blockscape_core::network::client::ClientMsg;

/// A quick shorthand for testing the enum type of an object
macro_rules! matches(
    ($e:expr, $p:pat) => (
        match $e {
            $p => true,
            _ => false
        }
    )
);

/// Using the given network ID, starts a mining/forging instance to attempt to sign a block for acceptance into the network.
pub fn start_forging(context: &Rc<Context>, handler: &Handle, _network_id: U256) {

    let (tx, rx) = mpsc::channel(10);

    // manufacture a fake event to get the miner started
    let tx = tx.send(RecordEvent::NewBlock { uncled: false, block: context.rk.get_current_block().expect("No current block!") }).wait()
        .expect("Could not post starting event for forging system");

    context.rk.register_record_listener(tx);

    let h2 = handler.clone();

    let context = Rc::clone(&context);

    let forge_response_task = rx
    .filter(|e| matches!(*e, RecordEvent::NewBlock { .. }))
    .for_each(move |e| -> Box<Future<Item=(), Error=()>> {

        if let RecordEvent::NewBlock { uncled, .. } = e {
            if uncled {
                return Box::new(future::ok(()));
            }
        }
        else { unreachable!() }


        let h3 = h2.clone();

        // should we be forging atm?
        let fun = |should: Result<bool, futures::Canceled>| {
            if !should.unwrap_or(false) {
                return Box::new(future::ok::<(), ()>(()));
            }

            let r = context.rk.create_block();

            if let Err(e) = r {
                warn!("Could not generate block for forging: {}", e);
                return Box::new(future::ok::<(), ()>(()));
            }

            let b = r.unwrap();

            // time for new work
            let h2 = h2.clone();
            let ctx2 = Rc::clone(&context);
            // unfortunately we have to make another channel for each block since the function is called multiple times
            let (tx2, rx2) = mpsc::channel(10);
            context.rk.register_record_listener(tx2);

            // we only want to stop the miner for new blocks on the main chain
            let rx2 = rx2.filter(|e| {
                if let &RecordEvent::NewBlock { ref uncled, .. } = e {
                    !uncled
                }
                else {
                    false
                }
            });

            let t = Box::new(rx2.into_future().select2(context.forge_algo.create(b)).then(move |r| {
                // did we get a block? submit if we did
                match r {
                    Ok(Either::B((block, _))) => {
                        let block = block.sign(&ctx2.forge_key);
                        let rk = Arc::clone(&ctx2.rk);
                        h2.spawn(ctx2.rk.get_priority_worker().spawn_fn(move || {
                            let r = rk.add_block(&block);
                            if let Ok(_) = r {
                                println!("FORGE: Submitted {} was accepted!", block.calculate_hash());
                            }
                            else {
                                println!("FORGE: Submitted {} was rejected: {:?}", block.calculate_hash(), r.unwrap_err());
                            }

                            Ok::<(), ()>(())
                        }).map(|_| ()).map_err(|_| ()));
                    },
                    Err(Either::B((err, _))) => {
                        warn!("Forging algorithm had an error: {:?}", err);
                    },
                    _ => {}
                }

                Ok::<(), ()>(())
            }));

            h2.spawn(t);

            Box::new(future::ok(()))
        };

        let (tx, rx) = oneshot::channel();
        if let Some(n) = context.network {
            n.unbounded_send(ClientMsg::ShouldForge(U256_ZERO, tx));
            h2.spawn(rx.then(fun));
        }
        else {
            h2.spawn(future::ok(true).then(fun));
        }

        Box::new(future::ok::<(), ()>(()))

    })
    .or_else(|e| {
        warn!("Failed to check sessions in timer: {:?}", e);

        future::err(())
    });

    debug!("Spawned the block forger");

    handler.spawn(forge_response_task);
}