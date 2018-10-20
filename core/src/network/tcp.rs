use std::mem::size_of;
use std::io;

use bincode;
use bytes::BytesMut;

use network::protocol::Packet;

use tokio_io::codec::Encoder;
use tokio_io::codec::Decoder;

pub struct TCPCodec;

impl Encoder for TCPCodec {
    type Item = Packet;
    type Error = io::Error;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), io::Error> {

        let payload = bincode::serialize(&item, bincode::Infinite).unwrap();

        dst.reserve(size_of::<u32>() + payload.len());

        dst.extend_from_slice(&bincode::serialize(&(payload.len() as u32), bincode::Infinite)
            .expect("could not serialize u32 integer"));
        dst.extend_from_slice(&payload[..]);

        Ok(())
    }
}

impl Decoder for TCPCodec {
    type Item = Packet;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Packet>, io::Error> {
        if src.len() < size_of::<u32>() {
            return Ok(None);
        }

        let size = bincode::deserialize::<u32>(&src[..size_of::<u32>()]).unwrap() as usize;

        if src.len() >= size + size_of::<u32>() {
            let d = src.split_to(size + size_of::<u32>());

            bincode::deserialize(&d[size_of::<u32>()..])
				.map(|p| Some(p))
				.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
        else {
            Ok(None)
        }
    }
}
