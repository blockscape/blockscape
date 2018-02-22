use std::sync::Arc;

use futures::sync::mpsc::UnboundedSender;

use openssl::pkey::PKey;

use blockscape_core::network::client::ClientMsg;
use blockscape_core::record_keeper::RecordKeeper;
use blockscape_core::forging::BlockForger;
use blockscape_core::hash::hash_pub_key;
use blockscape_core::primitives::U160;

use game::CheckersGame;

pub struct Context {
    pub network: Option<UnboundedSender<ClientMsg>>,
    pub rk: Arc<RecordKeeper>,
    pub game: Arc<CheckersGame>,
    pub forge_algo: Box<BlockForger>,

    pub forge_key: PKey
}

impl Context {
    #[inline]
    pub fn key_hash(&self) -> U160 {
        hash_pub_key(&self.forge_key.public_key_to_der().unwrap())
    }
}