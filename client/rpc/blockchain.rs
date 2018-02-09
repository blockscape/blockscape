use jsonrpc_core::*;
use jsonrpc_core::error::Error;
use jsonrpc_macros::IoDelegate;
use rpc::types::*;
use serde::Serialize;
use std::result::Result;
use std::sync::Arc;

use blockscape_core::bin::*;
use blockscape_core::primitives::*;
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::record_keeper::Error as RKErr;

pub struct BlockchainRPC {
    rk: Arc<RecordKeeper>
}

impl BlockchainRPC {
    pub fn add(rk: Arc<RecordKeeper>, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) -> Arc<BlockchainRPC> {
        let rpc = Arc::new(BlockchainRPC { rk });

        let mut d = IoDelegate::<BlockchainRPC, SocketMetadata>::new(rpc.clone());
        d.add_method_with_meta("add_block", Self::add_block);
        d.add_method_with_meta("add_pending_txn", Self::add_pending_txn);
        d.add_method_with_meta("get_validator_key", Self::get_validator_key);
        d.add_method_with_meta("get_validator_rep", Self::get_validator_rep);
        d.add_method_with_meta("get_current_block_hash", Self::get_current_block_hash);
        d.add_method_with_meta("get_current_block_header", Self::get_current_block_header);
        d.add_method_with_meta("get_current_block", Self::get_current_block);
        d.add_method_with_meta("get_block_height", Self::get_block_height);
        d.add_method_with_meta("get_blocks_of_height", Self::get_blocks_of_height);
        d.add_method_with_meta("get_latest_blocks", Self::get_latest_blocks);
        d.add_method_with_meta("get_plot_events", Self::get_plot_events);
        d.add_method_with_meta("get_block_header", Self::get_block_header);
        d.add_method_with_meta("get_block", Self::get_block);
        d.add_method_with_meta("get_txn", Self::get_txn);
        d.add_method_with_meta("get_txn_block", Self::get_txn_block);

        io.extend_with(d);
        rpc
    }

    fn add_block(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let block = expect_one_arg::<JBlock>(params)?.into();
        to_rpc_res(self.rk.add_block(&block, true))
    }

    fn add_pending_txn(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let txn = expect_one_arg::<JTxn>(params)?.into();
        to_rpc_res(self.rk.add_pending_txn(txn, true))
    }

    fn get_validator_key(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let id = expect_one_arg::<JU160>(params)?.into();
        into_rpc_res::<_, JBin>(self.rk.get_validator_key(&id))
    }

    fn get_validator_rep(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let id = expect_one_arg::<JU160>(params)?.into();
        to_rpc_res(self.rk.get_validator_rep(&id))
    }

    fn get_current_block_hash(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        into_rpc_res::<_, JU256>(Ok(self.rk.get_current_block_hash()))
    }

    fn get_current_block_header(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        into_rpc_res::<_, JBlockHeader>(self.rk.get_current_block_header())
    }

    fn get_current_block(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        into_rpc_res::<_, JBlock>(self.rk.get_current_block())
    }

    fn get_block_height(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        to_rpc_res(self.rk.get_block_height(&hash))
    }

    fn get_blocks_of_height(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let height = expect_one_arg(params)?;
        to_rpc_res(
            self.rk.get_blocks_of_height(height)
            .map(|v| v.into_iter().map(Into::into).collect::<Vec<JU256>>())
        )
    }

    fn get_latest_blocks(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let count = expect_one_arg(params)?;
        if count > 100 { Err(Error::invalid_params("Count too large.")) }
        else {
            to_rpc_res(
                self.rk.get_latest_blocks(count)
                .map(|v| v.into_iter().map(Into::into).collect::<Vec<JBlockHeader>>())
            )
        }
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
        let hash = expect_one_arg::<JU256>(params)?.into();
        into_rpc_res::<_, JBlock>(self.rk.get_block(&hash))
    }

    fn get_txn(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        into_rpc_res::<_, JTxn>(self.rk.get_txn(&hash))
    }

    fn get_txn_block(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        to_rpc_res(self.rk.get_txn_block(hash).map(|o| o.map(|h| JU256::from(h))))
    }
}

#[inline]
fn to_rpc_res<T: Serialize>(r: Result<T, RKErr>) -> RpcResult {
    r.map(|v| to_value(v).unwrap())
     .map_err(map_rk_err)
}

#[inline]
fn into_rpc_res<T, J>(r: Result<T, RKErr>) -> RpcResult
    where J: From<T> + Serialize
{
    to_rpc_res::<J>( r.map(|v| v.into()) )
}