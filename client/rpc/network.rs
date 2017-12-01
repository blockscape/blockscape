use std::sync::Arc;

use jsonrpc_core::*;

use blockscape_core::network::client::Client;

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

        io.extend_with(d);


        rpc
    }

    fn get_net_stats(&self, params: Params, meta: SocketMetadata) -> RpcResult {
        Ok(Value::String("Good morning.".into()))
    }
}