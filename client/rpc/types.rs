use std::net::SocketAddr;
use std::time::Instant;

use jsonrpc_core;
use jsonrpc_core::futures::Future;

pub type RpcResult = Result<jsonrpc_core::Value, jsonrpc_core::Error>;

#[derive(Clone)]
pub struct SocketMetadata {
	addr: SocketAddr,
}

impl Default for SocketMetadata {
	fn default() -> Self {
		SocketMetadata { addr: "0.0.0.0:0".parse().unwrap() }
	}
}

impl SocketMetadata {
	pub fn addr(&self) -> &SocketAddr {
		&self.addr
	}
}

impl jsonrpc_core::Metadata for SocketMetadata { }

impl From<SocketAddr> for SocketMetadata {
	fn from(addr: SocketAddr) -> SocketMetadata {
		SocketMetadata { addr: addr }
	}
}

pub struct LogMiddleware;

impl jsonrpc_core::Middleware<SocketMetadata> for LogMiddleware {
    type Future = jsonrpc_core::FutureResponse;

    fn on_request<F, X>(&self, request: jsonrpc_core::Request, meta: SocketMetadata, next: F) -> jsonrpc_core::FutureResponse where
		F: FnOnce(jsonrpc_core::Request, SocketMetadata) -> X + Send,
		X: Future<Item=Option<jsonrpc_core::Response>, Error=()> + Send + 'static,
	{
        let start = Instant::now();
		debug!("Processing RPC request: {:?}", request);

		Box::new(next(request, meta).map(move |res| {
			println!("Processing took: {:?}", start.elapsed());
			res
		}))
    }
}