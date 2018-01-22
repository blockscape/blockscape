use base16;
use std::fmt;

use serde::de;
use serde::de::*;
use serde::ser::{Serialize, Serializer};
use bincode::{serialize, Bounded};

use primitives::{U160, U256};

pub type Bin = Vec<u8>;

pub trait AsBin {
    fn as_bin(&self) -> Bin;
}

impl AsBin for U160 {
    fn as_bin(&self) -> Bin { self.to_vec() }
} impl AsBin for U256{
    fn as_bin(&self) -> Bin { self.to_vec() }
} impl AsBin for u64 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(8)).unwrap() }
} impl AsBin for i64 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(8)).unwrap() }
} impl AsBin for u32 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(4)).unwrap() }
} impl AsBin for i32 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(4)).unwrap() }
} impl AsBin for u16 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(2)).unwrap() }
} impl AsBin for i16 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(2)).unwrap() }
} impl AsBin for u8 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(1)).unwrap() }
} impl AsBin for i8 {
    fn as_bin(&self) -> Bin { serialize(self, Bounded(1)).unwrap() }
}


pub struct JBin(Vec<u8>);

impl From<Bin> for JBin {
    fn from(b: Bin) -> JBin {
        JBin(b)
    }
}

impl Into<Bin> for JBin {
    fn into(self) -> Bin {
        self.0
    }
}

impl Serialize for JBin {
    fn serialize<S>(&self, serilizer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serilizer.serialize_str(&base16::from_bin(&self.0))
    }
}

impl<'de> Deserialize<'de> for JBin {
    fn deserialize<D>(deserialize: D) -> Result<JBin, D::Error>
        where D: Deserializer<'de>
    {
        struct StrVisitor;
        
        impl<'de> Visitor<'de> for StrVisitor {
            type Value = JBin;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string")
            }

            fn visit_str<E>(self, v: &str) -> Result<JBin, E>
                where E: de::Error
            {
                base16::to_bin(v)
                    .map(|v| JBin(v))
                    .map_err(Error::custom)
            }
        }

        deserialize.deserialize_string(StrVisitor)
    }
}