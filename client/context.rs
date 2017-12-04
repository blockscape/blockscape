use std::sync::Arc;

use blockscape_core::network::client::Client;
use blockscape_core::record_keeper::RecordKeeper;

#[derive(Clone)]
pub struct Context {
    pub network: Option<Arc<Client>>,
    pub rk: Arc<RecordKeeper>
}