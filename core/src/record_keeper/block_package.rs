use primitives::{Block, BlockHeader, Txn, U256};
use bincode;

/// Estimated block size in bytes, this should be slightly under the true value, and will be used 
const ESTIMATED_BLOCK_SIZE: usize = 128;

/// A self-contained package of blocks and their associated transactions designed for easy transfer
/// between systems. This takes in reference to the database for construction to prevent other code
/// form being responsible for an invalid ordering of blocks or the txn references internally
/// stored.
#[derive(Debug, Serialize, Deserialize)]
struct BlockPackage {
    /// Blocks and their associated transactions sorted from the lowest height to the greatest height.
    blocks: Vec<(BlockHeader, Vec<u16>)>,
    /// The txns which are referenced by at least one of the included blocks.
    txns: Vec<Txn>
}

impl From<&[u8]> for BlockPackage {
    fn from(data: &[u8]) -> BlockPackage {

    }
}

impl BlockPackage {
    /// Create a `BlockPackage` of the unknown blocks from the last known block until the desired
    /// block. It will never include the `last_known` or `target` blocks in the package. The `limit`
    /// is the maximum number of bytes the final package may contain.
    ///
    /// In summary, it will always find the latest common ancestor of the two blocks and then
    /// traverse upwards until it reaches the target and only include those found when traversing
    /// upwards.
    pub fn unknown_blocks(db: &Database, last_known: &U256, target: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let blocks = db.get_unknown_blocks(last_known, target, limit / ESTIMATED_BLOCK_SIZE)?;
        Self::package(db, blocks, limit)
    }

    /// Create a `BlockPackage` of all the blocks of the current chain which are a descendent of the
    /// latest common ancestor between the chain of the start block and the current chain. It will
    /// not include the start block. The `limit` is the maximum number of bytes the final package
    /// may contain.
    pub fn blocks_after_hash(db: &Database, start: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let blocks = db.get_blocks_after_hash(start, limit / ESTIMATED_BLOCK_SIZE)?;
        Self::package(db, blocks, limit)
    }

    /// Take a list of block headers and package them.
    ///
    /// # Preconditions
    /// Headers should be in order from lowest height to greatest height, this will prevent needing
    /// to look up information that would have already been used to construct the lits in the first
    /// place.
    fn package(db: &Database, headers: Vec<BlockHeader>, limit: usize) -> Result<BlockPackage, Error> {
        // Start by getting all of the blocks for the headers and 
        let mut count: u16 = 0;
        let mut txns: HashMap<U256, u16> = HashMap::new();
        let blocks: Vec<Block> = headers.into_iter()
            .map(|header| db.complete_block(header))
            .collect();
        
        for block in blocks {
            for txn in block.txns {
                
            }
        }
    }

    /// Convert the `BlockPackage` into a compressed binary representation which can be easily
    /// transferred or archived.
    pub fn pack(self) -> Vec<u8> {

    }
}