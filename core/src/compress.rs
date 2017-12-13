use std::io::prelude::*;
use std::io;

use flate2::read::{GzEncoder,GzDecoder};
use flate2::Compression;


/// Returns a compressed version of the provided byte slice using the gzip compression algorithm, fastest encoding
pub fn compress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut e = GzEncoder::new(data, Compression::default());
    
    let mut out = Vec::new();
    e.read_to_end(&mut out).map(|_| out)
}

/// Returns the decompressed data of the provided compressed byte slice using the gzip decompression algorithm
pub fn decompress(compressed: &[u8]) -> io::Result<Vec<u8>> {
    let mut d  = GzDecoder::new(compressed)?;

    let mut out = Vec::new();
    d.read_to_end(&mut out).map(|_| out)
}