use jsonrpc_core::*;
use jsonrpc_core::error::Error;
use jsonrpc_macros::IoDelegate;
use rpc::types::*;
use serde::Serialize;
use std::result::Result;
use std::sync::Arc;
use openssl::pkey::PKey;

use blockscape_core::bin::*;
use blockscape_core::primitives::*;
use blockscape_core::time::Time;
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::record_keeper::Error as RKErr;
use blockscape_core::hash::hash_pub_key;

pub struct BlockchainRPC {
    rk: Arc<RecordKeeper>,
    forge_key: PKey
}

#[derive(Serialize)]
struct BlockRPC {
    header: JBlockHeader,

    txns: Vec<JU256>,

    height: u64,

    status: String,

    next: JU256
}

impl BlockRPC {
    pub fn new(block: Block, rk: &Arc<RecordKeeper>) -> BlockRPC {

        let block_hash = block.calculate_hash();

        let h = rk.get_block_height(&block_hash).expect("Could not load current block height from database!");
        let nh = rk.get_blocks_of_height(h + 1).expect("Blocks of height not available").first().unwrap_or(&U256_ZERO).clone();

        let status = match rk.is_block_in_current_chain(&block_hash) {
            Ok(true) => "Mainchain",
            Ok(false) => "Uncle",
            _ => ""
        };

        BlockRPC {
            header: block.get_header().clone().into(),
            txns: block.txns.into_iter().map(|n| n.into()).collect(),
            height: h,
            status: status.into(),
            next: nh.into()
        }
    }
}

#[derive(Serialize)]
struct TxnRPC {
    timestamp: Time,
    creator: JU160,
    mutation: JMutation,
    signature: JBin
}

impl TxnRPC {
    pub fn new(txn: Txn, _rk: &Arc<RecordKeeper>) -> TxnRPC {
        TxnRPC {
            timestamp: txn.timestamp,
            creator: txn.creator.into(),
            mutation: txn.mutation.into(),
            signature: txn.signature.into()
        }
    }
}

impl BlockchainRPC {
    pub fn add(rk: Arc<RecordKeeper>, forge_key: PKey, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) -> Arc<BlockchainRPC> {
        let rpc = Arc::new(BlockchainRPC { rk, forge_key });

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

        d.add_method_with_meta("sign_txn", Self::sign_txn);
        d.add_method_with_meta("sign_block", Self::sign_block);

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
        into_rpc_res::<_, BlockRPC>(self.rk.get_current_block().map(|b| BlockRPC::new(b, &self.rk)))
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
        into_rpc_res::<_, BlockRPC>(self.rk.get_block(&hash).map(|b| BlockRPC::new(b, &self.rk)))
    }

    fn get_txn(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        into_rpc_res::<_, TxnRPC>(self.rk.get_txn(&hash).map(|t| TxnRPC::new(t, &self.rk)))
    }

    fn get_txn_block(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        to_rpc_res(self.rk.get_txn_block(hash).map(|o| o.map(|h| JU256::from(h))))
    }


    fn sign_block(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let mut block : Block = expect_one_arg::<JBlock>(params)?.into();
        block.merkle_root = Block::calculate_merkle_root(&block.txns);
        block.creator = hash_pub_key(&self.forge_key.public_key_to_der().unwrap());
        block = block.sign(&self.forge_key);
        self.rk.is_valid_block(&block).map_err(map_rk_err)?;
        to_rpc_res(Ok((JU256::from(block.calculate_hash()), JBlock::from(block))))
    }

    fn sign_txn(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let mut txn : Txn = expect_one_arg::<JTxn>(params)?.into();
        txn.creator = hash_pub_key(&self.forge_key.public_key_to_der().unwrap());
        txn = txn.sign(&self.forge_key);
        self.rk.is_valid_txn(&txn).map_err(map_rk_err)?;
        to_rpc_res(Ok((JU256::from(txn.calculate_hash()), JTxn::from(txn))))
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