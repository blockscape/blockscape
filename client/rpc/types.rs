use std::net::SocketAddr;
use std::time::Instant;
use std::ops::Range;

use jsonrpc_core;
use jsonrpc_core::futures::Future;
use jsonrpc_core::error::Error;
use jsonrpc_core::Params;
use serde_json::{Value, from_value};
use serde::de::DeserializeOwned;

use blockscape_core::record_keeper::Error as RKErr;

pub type RpcResult = Result<jsonrpc_core::Value, jsonrpc_core::Error>;
pub type RpcFuture = Box<Future<Item=jsonrpc_core::Value, Error=jsonrpc_core::Error> + Send>;


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

pub fn expect_array(p: Params, size: Range<usize>) -> Result<Vec<Value>, Error> {
	match p {
		Params::Array(a) => {
			let len = a.len();
			if (len >= size.start) && (len < size.end) { Ok(a) }
			else { Err(Error::invalid_params("Incorrect number of arguments.")) }
		},
		_ => Err(Error::invalid_params("Expected array."))
	}
}

/*pub fn expect_map(p: Params) -> Result<Map<String, Value>, Error> {
	match p {
		Params::Map(m) => Ok(m),
		_ => Err(Error::invalid_params("Expected map.")),
	}
}*/

pub fn parse_args_simple<T: DeserializeOwned>(p: Params, size: Range<usize>) -> Result<Vec<T>, Error> {
	let vals = expect_array(p, size)?;
	let mut res = Vec::with_capacity(vals.len());

	for val in vals.into_iter() {
		res.push(
			from_value::<T>(val)
			.map_err(|e| Error::invalid_params(format!("{:?}", e)))?
		);
	} Ok(res)
}


pub fn expect_one_arg<T: DeserializeOwned>(p: Params) -> Result<T, Error> {
	let mut array = expect_array(p, (1..2))?;
	from_value(array.pop().unwrap())
		.map_err( |e| Error::invalid_params(format!("{:?}", e)) )
}

pub fn expect_two_args<A, B>(p: Params) -> Result<(A, B), Error>
	where A: DeserializeOwned,
		  B: DeserializeOwned
{
	let mut array = expect_array(p, (2..3))?;
	let b = from_value(array.pop().unwrap())
		.map_err( |e| Error::invalid_params(format!("{:?}", e)) )?;
	let a = from_value(array.pop().unwrap())
		.map_err( |e| Error::invalid_params(format!("{:?}", e)) )?;

	Ok((a, b))
}

pub fn map_rk_err(e: RKErr) -> Error {
    match e {
        RKErr::DB(..) => Error::internal_error(),
        RKErr::Deserialize(msg) => Error::invalid_params(msg),
        RKErr::Logic(err) => Error::invalid_params(format!("{:?}", err)),
		RKErr::OutOfMemory(msg) => Error::invalid_params(msg),
        RKErr::NotFound(..) => Error::invalid_request()
    }
}

/*pub fn read_value<T: DeserializeOwned>(m: &mut Map<String, Value>, key: &'static str) -> Result<T, Error> {	
	let v = m.remove(key).ok_or(Error::invalid_params(format!("Expected field '{}'.", key)))?;
	from_value::<T>(v).map_err( |e| Error::invalid_params(format!("{:?}", e)) )
}

pub fn read_opt_value<T: DeserializeOwned>(m: &mut Map<String, Value>, key: &'static str) -> Result<Option<T>, Error> {	
	if let Some(v) = m.remove(key) {
		from_value::<T>(v)
			.map(|v| Some(v))
			.map_err( |e| Error::invalid_params(format!("{:?}", e)) )
	} else { Ok(None) }
}*/

/// Like `try!`, but with futures!
macro_rules! tryf(
    ($e:expr) => {
		use futures::future;
        match $e {
            Ok(k) => k,
            Err(e) => return Box::new(future::err(e))
        }
    }
);