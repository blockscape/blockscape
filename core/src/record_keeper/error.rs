use rocksdb::Error as RocksDBError;
use std::error::Error as StdErr;
use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    DB(RocksDBError), // when there is an error working with the database itself
    NotFound(&'static [u8], &'static [u8], Vec<u8>), // when data is not found in the database
    Deserialize(String), // when data cannot be deserialized
    InvalidMut(String) // when a rule is broken by a mutation
}

impl StdErr for Error {
    fn description(&self) -> &str {
        match *self { //TODO: why can we just get a ref of the objects
            Error::DB(_) => "RocksDB error: aka, not my fault ☺",
            Error::NotFound(_, _, _) => "Could not find the data requested at that Hash (may not be an issue).",
            Error::Deserialize(_) => "Deserialization error, the data stored could not be deserialized into the requested type.",
            Error::InvalidMut(_) => "Invalid Mutation, a rule is violated by the mutation so it will not be applied."
        }
    }

    fn cause(&self) -> Option<&StdErr> {
        match *self {
            Error::DB(ref e) => Some(e),
            Error::NotFound(_, _, _) => None,
            Error::Deserialize(_) => None,
            Error::InvalidMut(_) => None,
        }
    }
}

impl From<RocksDBError> for Error {
    fn from(e: RocksDBError) -> Self { Error::DB(e) }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.description())
    }
}