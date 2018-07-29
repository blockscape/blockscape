use primitives::Block;
use record_keeper::database::Database;
use record_keeper::{Error, LogicError, DBState};
use record_keeper::rules::BlockRule;
use time::Time;

/// The time stamp must not be after the current time and must be after the previous block.
pub struct TimeStamp;
impl BlockRule for TimeStamp {
    fn is_valid(&self, _state: &DBState, db: &Database, block: &Block) -> Result<(), Error> {
        if (block.timestamp > Time::current()) ||
           (block.timestamp < db.get_block_header(&block.prev)?.timestamp)
        { Err(LogicError::InvalidTime.into()) } else { Ok(()) }
    }

    fn description(&self) -> &'static str {
        "The time stamp must be after the previous block and before the current time."
    }
}