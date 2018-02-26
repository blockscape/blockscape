use std::sync::Arc;

use jsonrpc_core::*;

use rpc::types::*;

use jsonrpc_macros::IoDelegate;

use rpc::types::RpcResult;

use libc::{getpid, kill, SIGTERM};

pub struct ControlRPC {
}

impl RPCHandler for ControlRPC {
    fn add(this: &Arc<ControlRPC>, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) {

        let mut d = IoDelegate::<ControlRPC, SocketMetadata>::new(this.clone());
        d.add_method_with_meta("stop", ControlRPC::stop);

        io.extend_with(d);
    }
}

impl ControlRPC {

    pub fn new() -> Arc<ControlRPC> {
        let rpc = Arc::new(ControlRPC {});

        rpc
    }

    fn stop(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        // initiate the stop process

        unsafe {
            // run the kill signal on our self, which will start the stop process
            kill(getpid(), SIGTERM);
        }

        Ok(serde_json::to_value(true).unwrap())
    }
}