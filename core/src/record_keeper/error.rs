use bincode::Error as BincodeError;
use rocksdb::Error as RocksDBError;
use std::error::Error as StdErr;
use std::fmt;
use std::fmt::Display;
use record_keeper::database as DB;

#[derive(Clone, Debug)]
pub enum Error {
    DB(RocksDBError), // when there is an error working with the database itself
    NotFound(DB::Key), // when data is not found in the database (prefix, key, postfix).
    Deserialize(String), // when data cannot be deserialized
    Logic(LogicError), // When something is wrong with a block, txn, or mutation
}

impl StdErr for Error {
    fn description(&self) -> &str {
        match *self { //TODO: why can we just get a ref of the objects
            Error::DB(_) => "RocksDB error: aka, not my fault ☺",
            Error::NotFound(_) => "Could not find the data requested at that Hash (may not be an issue).",
            Error::Deserialize(ref e) => e,
            Error::Logic(_) => "Something is not right with the block, txn, or mutations."
        }
    }

    fn cause(&self) -> Option<&StdErr> {
        match *self {
            Error::DB(ref e) => Some(e),
            Error::NotFound(..) => None,
            Error::Deserialize(_) => None,
            Error::Logic(ref e) => Some(e),
        }
    }
}

impl From<RocksDBError> for Error {
    fn from(e: RocksDBError) -> Self { Error::DB(e) }
}

impl From<BincodeError> for Error {
    fn from(e: BincodeError) -> Self { Error::Deserialize(e.to_string()) }
}

impl From<LogicError> for Error {
    fn from(e: LogicError) -> Self { Error::Logic(e) }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.description())
    }
}

/// Map a Result of type <T, Error> to <T, Error> setting an Error::NotFound to be val.
#[inline]
pub fn map_not_found<T>(res: Result<T, Error>, val: T) -> Result<T, Error> {
    match res {
        Ok(v) => Ok(v),
        Err(Error::NotFound(..)) => Ok(val),
        Err(e) => Err(e)
    }
}


#[derive(Clone, Debug)]
pub enum LogicError {
    Duplicate,
    InvalidMerkleRoot,
    InvalidMutation(String),
    InvalidSignature,
    InvalidTime,
    MissingPrevious,
    UndoOrigin,
    UnrecognizedCreator,
}

impl StdErr for LogicError {
    fn description(&self) -> &str {
        match *self {
            LogicError::Duplicate => "This has already been accepted into the blockchain.",
            LogicError::InvalidMerkleRoot => "The merkle_root does not match the txn list.",
            LogicError::InvalidMutation(_) => "The mutation breaks a rule.",
            LogicError::InvalidSignature => "The data does not match the signature.",
            LogicError::InvalidTime => "The timestamp is after the current time or too long ago.",
            LogicError::MissingPrevious => "The last block this references is not known to us.",
            LogicError::UndoOrigin => "Cannot walk backwards past an origin block.",
            LogicError::UnrecognizedCreator => "The person who created and signed the block is unknown.",
        }
    }

    fn cause(&self) -> Option<&StdErr> {
        None
    }
}

impl Display for LogicError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.description())
    }
}