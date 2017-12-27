use base16;
use std::fmt;

use serde::de;
use serde::de::*;
use serde::ser::{Serialize, Serializer};


pub type Bin = Vec<u8>;
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