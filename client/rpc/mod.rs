pub mod client;

mod types;

mod blockchain;
mod control;
mod network;

use context::Context;
use jsonrpc_core::*;
use jsonrpc_http_server::{ServerBuilder, Server};
use rpc::types::LogMiddleware;
use std::net::SocketAddr;

use rpc::blockchain::BlockchainRPC;
use rpc::control::ControlRPC;
use rpc::network::NetworkRPC;

pub struct RPC {
    server: Server,
}

impl RPC {

    pub fn run(bind_addr: SocketAddr, ctx: Context) -> RPC {
        let mut io = MetaIoHandler::with_middleware(LogMiddleware);

        ControlRPC::add(&mut io);

        if let Some(net_client) = ctx.network {
            NetworkRPC::add(net_client.clone(), &mut io);
        }

        RPC {
            server: ServerBuilder::new(io).start_http(&bind_addr).expect("Could not start RPC Interface")
        }
    }

    pub fn close(self) {
        self.server.close();
    }
}