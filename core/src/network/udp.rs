use std::io;
use std::net::SocketAddr;

use bincode;
use tokio_core::net::UdpCodec;

use network::protocol::SocketPacket;

pub struct UDPCodec;

impl UdpCodec for UDPCodec {
    type In = SocketPacket;
    type Out = SocketPacket;

    fn decode(&mut self, src: &SocketAddr, buf: &[u8]) -> io::Result<Self::In> {
        Ok(SocketPacket(src.clone(), bincode::deserialize(buf).map_err(|_| io::ErrorKind::Other)?))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> SocketAddr {
        buf.extend(bincode::serialize(&msg.1, bincode::Infinite).unwrap());

        msg.0
    }
}
