use std::net::SocketAddr;
use std::time::Instant;

use jsonrpc_core;
use jsonrpc_core::futures::Future;
use jsonrpc_core::error::Error;
use jsonrpc_core::Params;

use serde_json::Value;

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

/*impl SocketMetadata {
	pub fn addr(&self) -> &SocketAddr {
		&self.addr
	}
}*/

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
			debug!("Processing took: {:?}", start.elapsed());
			res
		}))
    }
}

pub fn parse_args_simple(p: Params) -> Result<Vec<String>, jsonrpc_core::Error> {
	match p.parse() {
		Ok(Value::Array(vec)) => {

			let pv: Vec<Option<String>> = vec.into_iter().map(|v| v.as_str().map(|s| s.into())).collect();

			for v in &pv {
				if v.is_none() {
					return Err(Error::invalid_params("All parameters should be simple strings"));
				}
			}

			Ok(pv.into_iter().map(|v| v.unwrap()).collect())
		}
		_ => Err(Error::invalid_params("Could not parse or args missing"))
	}
}