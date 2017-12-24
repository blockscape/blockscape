use std::sync::Arc;

use jsonrpc_core::*;

use blockscape_core::network::client::*;

use blockscape_core::primitives::u256::*;

use rpc::types::*;

use jsonrpc_macros::IoDelegate;

use rpc::types::RpcResult;

pub struct NetworkRPC {
    net_client: Arc<Client>
}

impl NetworkRPC {
    pub fn add(client: Arc<Client>, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) -> Arc<NetworkRPC> {
        let rpc = Arc::new(NetworkRPC {
            net_client: client
        });

        let mut d = IoDelegate::<NetworkRPC, SocketMetadata>::new(rpc.clone());
        d.add_method_with_meta("get_net_stats", NetworkRPC::get_net_stats);
        d.add_method_with_meta("get_peer_info", NetworkRPC::get_peer_info);
        d.add_method_with_meta("attach_network", NetworkRPC::attach_network);
        d.add_method_with_meta("add_node", NetworkRPC::add_node);

        io.extend_with(d);


        rpc
    }

    fn get_net_stats(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        Ok(serde_json::to_value(self.net_client.get_stats()).unwrap())
    }

    fn get_peer_info(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        Ok(serde_json::to_value(self.net_client.get_peer_info()).unwrap())
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
            self.net_client.attach_network(network_id, mode)
                .map(|_| Value::Bool(true))
                .map_err(|_| Error::invalid_request())
        }
        else if op == "remove" {
            if self.net_client.detach_network(&network_id) {
                Ok(Value::Bool(true))
            }
            else {
                Err(Error::invalid_request())
            }
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