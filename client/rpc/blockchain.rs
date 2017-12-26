use jsonrpc_core::*;
use jsonrpc_macros::IoDelegate;
use rpc::types::*;
use std::sync::Arc;
use jsonrpc_core::error::Error;
use serde::Serialize;
use std::result::Result;

use blockscape_core::primitives::{U160};
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::record_keeper::Error as RKErr;

pub struct BlockchainRPC {
    rk: Arc<RecordKeeper>
}

impl BlockchainRPC {
    pub fn add(rk: Arc<RecordKeeper>, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) -> Arc<BlockchainRPC> {
        let rpc = Arc::new(BlockchainRPC { rk });

        let mut d = IoDelegate::<BlockchainRPC, SocketMetadata>::new(rpc.clone());
        d.add_method_with_meta("create_block", Self::create_block);
        d.add_method_with_meta("add_block", Self::add_block);
        d.add_method_with_meta("add_pending_txn", Self::add_pending_txn);
        d.add_method_with_meta("get_validator_key", Self::get_validator_key);
        d.add_method_with_meta("get_validator_rep", Self::get_validator_rep);
        d.add_method_with_meta("get_current_block_hash", Self::get_current_block_hash);
        d.add_method_with_meta("get_current_block_header", Self::get_current_block_header);
        d.add_method_with_meta("get_current_block", Self::get_current_block);
        d.add_method_with_meta("get_block_height", Self::get_block_height);
        d.add_method_with_meta("get_blocks_of_height", Self::get_blocks_of_height);
        d.add_method_with_meta("get_blocks_before", Self::get_blocks_before);
        d.add_method_with_meta("get_blocks_after", Self::get_blocks_after);
        d.add_method_with_meta("get_plot_events", Self::get_plot_events);
        d.add_method_with_meta("get_block_header", Self::get_block_header);
        d.add_method_with_meta("get_block", Self::get_block);
        d.add_method_with_meta("get_txn", Self::get_txn);

        io.extend_with(d);
        rpc
    }

    fn create_block(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        to_rpc_res(self.rk.create_block())
    }

    fn add_block(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let block = expect_one_arg(params)?;
        to_rpc_res(self.rk.add_block(&block))
    }

    fn add_pending_txn(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let txn = expect_one_arg(params)?;
        to_rpc_res(self.rk.add_pending_txn(&txn))
    }

    fn get_validator_key(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let id = expect_one_arg::<U160>(params)?;
        to_rpc_res(self.rk.get_validator_key(&id))
    }

    fn get_validator_rep(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let id = expect_one_arg(params)?;
        to_rpc_res(self.rk.get_validator_rep(&id))
    }

    fn get_current_block_hash(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        Ok(to_value(self.rk.get_current_block_hash()).unwrap())
    }

    fn get_current_block_header(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        to_rpc_res(self.rk.get_current_block_header())
    }

    fn get_current_block(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        to_rpc_res(self.rk.get_current_block())
    }

    fn get_block_height(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg(params)?;
        to_rpc_res(self.rk.get_block_height(&hash))
    }

    fn get_blocks_of_height(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let height = expect_one_arg(params)?;
        to_rpc_res(self.rk.get_blocks_of_height(height))
    }

    fn get_blocks_before(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let mut params = expect_map(params)?;
        let last_known = read_value(&mut params, "last_known")?;
        let target = read_value(&mut params, "target")?;
        let limit = read_opt_value(&mut params, "limit")?.unwrap_or(1000);
        to_rpc_res(self.rk.get_blocks_before(&last_known, &target, limit))
    }

    fn get_blocks_after(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let mut params = expect_map(params)?;
        let start = read_value(&mut params, "start")?;
        let limit = read_opt_value(&mut params, "limit")?.unwrap_or(1000);
        to_rpc_res(self.rk.get_blocks_after(&start, limit))
    }

    fn get_plot_events(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let (plot_id, after_tick) = expect_two_args(params)?;
        to_rpc_res(self.rk.get_plot_events(plot_id, after_tick))
    }

    fn get_block_header(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg(params)?;
        to_rpc_res(self.rk.get_block_header(&hash))
    }

    fn get_block(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg(params)?;
        to_rpc_res(self.rk.get_block(&hash))
    }

    fn get_txn(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg(params)?;
        to_rpc_res(self.rk.get_txn(&hash))
    }
}

fn to_rpc_res<T: Serialize>(r: Result<T, RKErr>) -> RpcResult {
    r.map(|v| to_value::<T>(v).unwrap())
     .map_err(map_rk_err)
}

fn map_rk_err(e: RKErr) -> Error {
    match e {
        RKErr::DB(..) => Error::internal_error(),
        RKErr::Deserialize(msg) => Error::invalid_params(msg),
        RKErr::Logic(err) => Error::invalid_params(format!("{:?}", err)),
        RKErr::NotFound(..) => Error::invalid_request()
    }
}