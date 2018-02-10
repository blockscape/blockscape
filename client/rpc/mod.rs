pub mod client;

#[macro_use]
mod types;

mod blockchain;
mod control;
mod network;
mod checkers;

use context::Context;
use jsonrpc_core::*;
use jsonrpc_http_server::{ServerBuilder, Server};
use rpc::types::LogMiddleware;
use std::net::SocketAddr;
use std::rc::Rc;
use openssl::pkey::PKey;

use rpc::blockchain::BlockchainRPC;
use rpc::control::ControlRPC;
use rpc::network::NetworkRPC;
use rpc::checkers::CheckersRPC;

pub struct RPC {
    server: Server,
}

impl RPC {

    pub fn run(bind_addr: SocketAddr, ctx: Rc<Context>) -> RPC {
        let mut io = MetaIoHandler::with_middleware(LogMiddleware);

        ControlRPC::add(&mut io);

        if let Some(ref net_client) = ctx.network {
            NetworkRPC::add(net_client.clone(), &mut io);
        }

        let forge_key = PKey::private_key_from_der(&ctx.forge_key.private_key_to_der().unwrap()).unwrap();
        BlockchainRPC::add(ctx.rk.clone(), forge_key, &mut io);
        CheckersRPC::add(
            ctx.game.clone(), ctx.key_hash(), &mut io);

        RPC {
            server: ServerBuilder::new(io).start_http(&bind_addr).expect("Could not start RPC Interface")
        }
    }

    pub fn close(self) {
        self.server.close();
    }
}