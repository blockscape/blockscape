pub mod client;

#[macro_use]
mod types;

mod blockchain;
mod control;
mod network;

use jsonrpc_http_server::{ServerBuilder, Server};
use std::net::SocketAddr;

pub use rpc::blockchain::BlockchainRPC;
pub use rpc::control::ControlRPC;
pub use rpc::network::NetworkRPC;

pub use rpc::types::*;

pub use jsonrpc_macros::IoDelegate;
pub use jsonrpc_core::Error;
pub use jsonrpc_core::MetaIoHandler;

pub struct RPC {
    server: Server,
}

impl RPC {

    pub fn build_handler() -> MetaIoHandler<SocketMetadata, LogMiddleware> {
        MetaIoHandler::with_middleware(LogMiddleware)
    }

    pub fn run(bind_addr: SocketAddr, handlers: MetaIoHandler<SocketMetadata, LogMiddleware>) -> RPC {
        RPC {
            server: ServerBuilder::new(handlers).start_http(&bind_addr).expect("Could not start RPC Interface")
        }
    }

    pub fn close(self) {
        self.server.close();
    }
}