use std::sync::Arc;

use futures::sync::mpsc::UnboundedSender;

use openssl::pkey::PKey;

use blockscape_core::network::client::ClientMsg;
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::forging::BlockForger;

use game::CheckersGame;

pub struct Context {
    pub network: UnboundedSender<ClientMsg>,
    pub rk: Arc<RecordKeeper>,
    pub game: Arc<CheckersGame>,
    pub forge_algo: Arc<BlockForger>,

    pub forge_key: PKey
}