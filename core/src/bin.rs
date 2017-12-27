use base16;
use std::fmt;
use std::ops::{Deref, DerefMut};

use serde::de;
use serde::de::*;
use serde::ser::{Serialize, Serializer};


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bin(Vec<u8>);

impl From<Vec<u8>> for Bin {
    fn from(v: Vec<u8>) -> Bin {
        Bin(v)
    }
}

impl Into<Vec<u8>> for Bin {
    fn into(self) -> Vec<u8> {
        self.0
    }
}

impl Deref for Bin {
    type Target = Vec<u8>;

    fn deref(&self) -> &Vec<u8> {
        &self.0
    }
}

impl DerefMut for Bin {
    fn deref_mut(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

impl Bin {
    #[inline]
    pub fn new() -> Bin {
        Vec::new().into()
    }

    #[inline]
    pub fn with_capacity(s: usize) -> Bin {
        Vec::with_capacity(s).into()
    }
}



pub struct JBin(Vec<u8>);

impl From<Bin> for JBin {
    fn from(b: Bin) -> JBin {
        JBin(b.0)
    }
}

impl Into<Bin> for JBin {
    fn into(self) -> Bin {
        Bin(self.0)
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