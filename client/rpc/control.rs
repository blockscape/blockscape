use std::sync::Arc;

use jsonrpc_core::*;

use rpc::types::*;

use jsonrpc_macros::IoDelegate;

use rpc::types::RpcResult;

use libc::{getpid, kill, SIGTERM};

pub struct ControlRPC {
}

impl ControlRPC {
    pub fn add(io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) -> Arc<ControlRPC> {
        let rpc = Arc::new(ControlRPC {
        });

        let mut d = IoDelegate::<ControlRPC, SocketMetadata>::new(rpc.clone());
        d.add_method_with_meta("stop", ControlRPC::stop);

        io.extend_with(d);


        rpc
    }


    fn stop(&self, params: Params, meta: SocketMetadata) -> RpcResult {
        // initiate the stop process

        unsafe {
            // run the kill signal on our self, which will start the stop process
            kill(getpid(), SIGTERM);
        }

        Ok(serde_json::to_value(true).unwrap())
    }
}