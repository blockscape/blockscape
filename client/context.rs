use std::sync::Arc;

use futures::sync::mpsc::UnboundedSender;

use blockscape_core::network::client::ClientMsg;
use blockscape_core::record_keeper::RecordKeeper;

#[derive(Clone)]
pub struct Context {
    pub network: Option<UnboundedSender<ClientMsg>>,
    pub rk: Arc<RecordKeeper>
}