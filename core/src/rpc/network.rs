use std::sync::Arc;

use jsonrpc_core::*;

use network::client::*;
use primitives::u256::*;

use rpc::types::*;

use jsonrpc_macros::IoDelegate;

use serde_json;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::sync::oneshot;

pub struct NetworkRPC {
    net_client: UnboundedSender<ClientMsg>
}

impl RPCHandler for NetworkRPC {
    fn add(this: &Arc<NetworkRPC>, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) {
        let mut d = IoDelegate::<NetworkRPC, SocketMetadata>::new(this.clone());
        d.add_method_with_meta("get_net_stats", NetworkRPC::get_net_stats);
        d.add_method_with_meta("get_peer_info", NetworkRPC::get_peer_info);
        d.add_method_with_meta("attach_network", NetworkRPC::attach_network);
        d.add_method_with_meta("add_node", NetworkRPC::add_node);

        io.extend_with(d);
    }
}

impl NetworkRPC {

    pub fn new(client: UnboundedSender<ClientMsg>) -> Arc<NetworkRPC> {
        let rpc = Arc::new(NetworkRPC {
            net_client: client
        });

        rpc
    }

    fn get_net_stats(&self, _params: Params, _meta: SocketMetadata) -> RpcFuture {
        let (tx, rx) = oneshot::channel();
        tryf!(self.net_client.unbounded_send(ClientMsg::GetStatistics(tx)).map_err(|_| Error::internal_error()));

        Box::new(rx
            .map_err(|_| Error::internal_error())
            .and_then(|n| serde_json::to_value(n)
                .map_err(|_| Error::internal_error())))

        //Ok(serde_json::to_value(self.net_client.get_stats()).unwrap())
    }

    fn get_peer_info(&self, _params: Params, _meta: SocketMetadata) -> RpcFuture {
        let (tx, rx) = oneshot::channel();
        tryf!(self.net_client.unbounded_send(ClientMsg::GetPeerInfo(tx)).map_err(|_| Error::internal_error()));

        Box::new(rx
            .map_err(|_| Error::internal_error())
            .and_then(|n| serde_json::to_value(n)
                .map_err(|_| Error::internal_error())))

        //Ok(serde_json::to_value(self.net_client.get_peer_info()).unwrap())
    }

    fn attach_network(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let args = parse_args_simple::<String>(params, (2..1000))?;

        let network_id: U256 = args[1].parse().unwrap_or(U256_ZERO);

        let op = args[0].as_str();

        if op == "add" {
            let mode = match args[2].as_str() {
                "primary" => ShardMode::Primary,
                "aux" => ShardMode::Auxillery,
                "queryonly" => ShardMode::QueryOnly,
                _ => {
                    return Err(Error::invalid_params(format!("Invalid network mode")))
                }
            };

            //let (tx, rx) = oneshot::channel();
            try!(self.net_client.unbounded_send(ClientMsg::AttachNetwork(network_id, mode)).map_err(|_| Error::internal_error()));

            // TODO: Better handling
            Ok(Value::Bool(true))
        }
        else if op == "remove" {
            //let (tx, rx) = oneshot::channel();
            try!(self.net_client.unbounded_send(ClientMsg::DetachNetwork(network_id)).map_err(|_| Error::internal_error()));

            // TODO: Better handling
            Ok(Value::Bool(true))
        }
        else {
            Err(Error::invalid_params(format!("Invalid operation: {}", op)))
        }
    }

    fn add_node(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let args = parse_args_simple::<String>(params, (1..4))?;

        if args.len() == 3 {
            Ok(Value::String("Not implemented".into()))
        }
        else if args.len() == 1 && args[0] == "help" {
            Ok(Value::String("Usage: add_node <host:port> <network_id> <add|remove>".into()))
        }
        else {
            Err(Error::invalid_params("Incorrect number of parameters"))
        }
    }
}