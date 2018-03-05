use jsonrpc_core::*;
use jsonrpc_core::error::Error;
use jsonrpc_macros::IoDelegate;
use rpc::types::*;
use serde::Serialize;
use std::result::Result;
use std::sync::Arc;
use openssl::pkey::PKey;
use std::collections::HashSet;

use bin::*;
use primitives::*;
use time::Time;
use record_keeper::RecordKeeper;
use record_keeper::Error as RKErr;
use hash::hash_pub_key;

pub struct BlockchainRPC {
    rk: Arc<RecordKeeper>,
    forge_key: PKey
}

#[derive(Serialize)]
struct BlockHeaderRPC {
    version: u16,

    timestamp: Time,

    shard: JU256,

    prev: JU256,

    merkle_root: JU256,

    creator: JU160,

    hash: JU256,

    txn_count: u64,

    height: u64,
}

impl BlockHeaderRPC {
    pub fn new(header: &BlockHeader, rk: &Arc<RecordKeeper>) -> Result<BlockHeaderRPC, RKErr> {
        let block_hash = header.calculate_hash();
        let h = rk.get_block_height(&block_hash)?;
        let block = rk.get_block(&block_hash)?;

        Ok(BlockHeaderRPC {
            version: header.version,
            timestamp: header.timestamp.into(),
            shard: header.shard.into(),
            prev: header.prev.into(),
            merkle_root: header.merkle_root.into(),
            creator: header.creator.into(),
            hash: block_hash.into(),
            txn_count: block.txns.len() as u64,
            height: h
        })
    }
}

#[derive(Serialize)]
struct BlockRPC {
    header: BlockHeaderRPC,

    txns: Vec<BlockTxnRPC>,

    status: String,

    next: JU256
}

impl BlockRPC {
    pub fn new(block: Block, rk: &Arc<RecordKeeper>) -> Result<BlockRPC, RKErr> {

        let block_hash = block.calculate_hash();

        let bh = BlockHeaderRPC::new(block.get_header(), rk)?;
        let nh = rk.get_blocks_of_height(bh.height + 1)?.first().unwrap_or(&U256_ZERO).clone();

        let status = match rk.is_block_in_current_chain(&block_hash) {
            Ok(true) => "Mainchain",
            Ok(false) => "Uncle",
            _ => ""
        };

        Ok(BlockRPC {
            header: bh,
            txns: block.txns.into_iter().map(|n| BlockTxnRPC::new(n, rk)).collect::<Result<Vec<_>, RKErr>>()?,
            status: status.into(),
            next: nh.into()
        })
    }
}

#[derive(Serialize)]
struct BlockTxnRPC {
    timestamp: Time,
    hash: JU256,
    size: u64,
    change_count: u64
}

impl BlockTxnRPC {
    pub fn new(hash: U256, rk: &Arc<RecordKeeper>) -> Result<BlockTxnRPC, RKErr> {

        let txn = rk.get_txn(&hash)?;

        Ok(BlockTxnRPC {
            timestamp: txn.timestamp,
            hash: hash.into(),
            size: txn.calculate_size() as u64,
            change_count: txn.mutation.changes.len() as u64,
        })
    }
}

#[derive(Serialize)]
struct TxnRPC {
    hash: JU256,
    timestamp: Time,
    creator: JU160,
    mutation: JMutation,
    signature: JBin,
    size: u64,
    block: Option<HashSet<JU256>>
}

impl TxnRPC {
    pub fn new(txn: Txn, rk: &Arc<RecordKeeper>) -> Result<TxnRPC, RKErr> {
        Ok(TxnRPC {
            hash: txn.calculate_hash().into(),
            timestamp: txn.timestamp,
            creator: txn.creator.into(),
            size: txn.calculate_size() as u64,
            block: rk.get_txn_blocks(txn.calculate_hash())
                .map(|o| o.map(|h| h.into_iter().map(Into::into).collect()))?,
            mutation: txn.mutation.into(),
            signature: txn.signature.into()
        })
    }
}

impl RPCHandler for BlockchainRPC {
    fn add(this: &Arc<BlockchainRPC>, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) {

        let mut d = IoDelegate::<BlockchainRPC, SocketMetadata>::new(this.clone());
        d.add_method_with_meta("get_chain_stats", Self::get_chain_stats);
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
        d.add_method_with_meta("get_txn_blocks", Self::get_txn_blocks);
        d.add_method_with_meta("get_account_txns", Self::get_account_txns);
        d.add_method_with_meta("get_txn_receive_time", Self::get_txn_receive_time);

        d.add_method_with_meta("sign_txn", Self::sign_txn);
        d.add_method_with_meta("sign_block", Self::sign_block);

        io.extend_with(d);
    }
}

impl BlockchainRPC {

    pub fn new(rk: Arc<RecordKeeper>, forge_key: PKey) -> Arc<BlockchainRPC> {
        let rpc = Arc::new(BlockchainRPC { rk, forge_key });

        rpc
    }

    fn get_chain_stats(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        to_rpc_res(self.rk.get_stats())
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
        into_rpc_res::<_, BlockRPC>(self.rk.get_current_block().map(|b| BlockRPC::new(b, &self.rk)).unwrap_or_else(|e| Err(e)))
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
            let r = self.rk.get_latest_blocks(count);
            to_rpc_res(
                if let Ok(blocks) = r {
                    blocks.into_iter().map(
                        |h| BlockHeaderRPC::new(&h, &self.rk)
                    ).collect::<Result<Vec<BlockHeaderRPC>, RKErr>>()
                }
                else {
                    Err(r.unwrap_err())
                }
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
        into_rpc_res::<_, BlockRPC>(self.rk.get_block(&hash).map(|b| BlockRPC::new(b, &self.rk)).unwrap_or_else(|e| Err(e)))
    }

    fn get_txn(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        into_rpc_res::<_, TxnRPC>(self.rk.get_txn(&hash).map(|t| TxnRPC::new(t, &self.rk)).unwrap_or_else(|e| Err(e)))
    }

    fn get_txn_blocks(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        to_rpc_res(self.rk.get_txn_blocks(hash)
           .map(|o| o.map(|b|
               b.into_iter()
               .map(JU256::from)
               .collect::<HashSet<JU256>>()
            ))
        )
    }

    fn get_account_txns(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU160>(params)?.into();
        to_rpc_res(self.rk.get_account_txns(hash).map(|k|
            k.into_iter()
           .map(JU256::from)
           .collect::<HashSet<JU256>>()
        ))
    }

    fn get_txn_receive_time(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let hash = expect_one_arg::<JU256>(params)?.into();
        to_rpc_res(self.rk.get_txn_receive_time(hash))
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
