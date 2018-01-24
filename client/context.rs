use std::sync::Arc;

use futures::sync::mpsc::UnboundedSender;

use openssl::pkey::PKey;

use blockscape_core::network::client::ClientMsg;
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::forging::BlockForger;

pub struct Context {
    pub network: Option<UnboundedSender<ClientMsg>>,
    pub rk: Arc<RecordKeeper>,
    pub forge_algo: Box<BlockForger>,

    pub forge_key: PKey
}