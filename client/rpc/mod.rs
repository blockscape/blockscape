pub mod client;

mod types;

mod control;
mod network;

use std::net::SocketAddr;

use rpc::types::LogMiddleware;

use rpc::control::ControlRPC;
use rpc::network::NetworkRPC;

use jsonrpc_core::*;
use jsonrpc_http_server::{ServerBuilder, Server};

use context::Context;

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