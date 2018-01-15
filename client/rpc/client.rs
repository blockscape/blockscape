// Miniature JSONRPC over HTTP client for the purposes of client RPC _ONLY_.

use std::net::SocketAddr;
//use std::io::{Write};
use std::io;
use std::fmt;
use std::time::Duration;
//use serde::{Serialize, Deserialize};
use serde_json;
use serde_json::Value;
use hyper::{Client, Method, Request, self};
use hyper::header::{ContentLength, ContentType, Accept};
use tokio_core::reactor::Core;
use tokio_core::reactor::Timeout;

use futures::{Future, Stream};

use futures::future::Either;


#[derive(Serialize, Deserialize)]
pub struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Value,
    id: u64
}

impl JsonRpcRequest {
    pub fn new(method: String, params: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: method,
            params: params,
            id: 0
        }
    }

    pub fn exec_sync(&self, host: SocketAddr) -> Result<JsonRpcResponse, hyper::Error> {

        let data = serde_json::to_string(self).map_err(|e| {

            hyper::Error::from(io::Error::new(
                io::ErrorKind::Other,
                e
            ))
        })?;

        let mut core = Core::new()?;
        let client = Client::new(&core.handle());

        let uri = format!("http://{}/", host).parse().expect("Could not post hostname for RPC server");

        let mut req = Request::new(Method::Post, uri);

        req.headers_mut().set(ContentType::json());
        req.headers_mut().set(ContentLength(data.len() as u64));
        req.headers_mut().set(Accept::json());

        req.set_body(data);

        let to = Timeout::new(Duration::from_secs(10), &core.handle()).unwrap();

        let r = client.request(req)
            .and_then(|res| serde_json::from_slice(&res.body().concat2().wait().unwrap())
                .map_err(|e| hyper::Error::Io(io::Error::new(io::ErrorKind::InvalidData, e))));

        let work = r.select2(to).then(|res| match res {
            Ok(Either::A(a)) => Ok(a.0),
            Ok(Either::B(_)) => Err(hyper::Error::Timeout),
            Err(Either::A(a)) => Err(a.0),
            Err(Either::B(b)) => Err(hyper::Error::from(b.0))
        });

        /*client.request(req).select2(to).wait()
            .map(|res| {
                match res {
                    Either::A(r) => serde_json::from_slice(&r.0.body().concat2().wait().unwrap()).map_err(|e| hyper::Error::Io(io::Error::new(io::ErrorKind::InvalidData, e))),
                    Either::B(_) => Err(hyper::Error::Timeout)
                }
            }).ok().unwrap()*/
        
        core.run(work)
    }
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcError {
    code: i64,
    message: String,
    data: Option<Value>
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<JsonRpcError>,
    id: u64
}

impl fmt::Display for JsonRpcResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref err) = self.error {
            write!(f, "Remote RPC Error {}: {:#}", err.code, err.message)
        }
        else {
            write!(f, "{:#}", self.result.clone().unwrap_or_default())
        }
    }
}