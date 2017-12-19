use bincode;
use compress::{compress, decompress};
use primitives::{Block, BlockHeader, Txn, U256};

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
    #[inline]
    fn from(data: &[u8]) -> BlockPackage {
        BlockPackage::unpack(data)
    }
}

impl BlockPackage {
    /// Create a new, empty blockpackage.
    fn new() -> BlockPackage {
        BlockPackage { blocks: Vec::new(), txns: Vec::new() }
    }

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
        let mut txns_by_hash: HashMap<U256, u16> = HashMap::new();
        let mut blocks: Vec<Block> = Vec::new();
        
        for header in headers.into_iter() {
            let block = db.complete_block(header)?;
            blocks.push(block);
        }
        
        // create an integer index for all of the transaction hashes 
        for block in blocks {
            for txn in block.txns {
                if !txns_by_hash.contains_key(&txn) {
                    txns_by_hash.insert(txn, count);
                    assert!(count < 0xffff);
                    count += 1;
                }
            }
        }

        // add one block at a time to the package and needed transactions
        count = 0; 
        let mut size: usize = 0;  // running byte count
        let mut package = Self::new();
        for block in blocks {
            let mut txn_indicies: Vec<u16> = Vec::new();
            let mut new_txns: Vec<Txn> = Vec::new();

            // size of block header and the txns, add one to list of txns to account for
            // a possible termination deliminer
            size += size_of(BlockHeader) + (block.txns.len() + 1) * 2;

            for txn in block.txns {
                let index = txns_by_hash.get(txn).unwrap();
                txn_indicies.append(index);
                
                if index == count { //we need to add the txn itself
                    let full_txn = db.get_txn(txn)?;
                    size += full_txn.calculate_size();
                    new_txns.push(full_txn);
                    count += 1;
                }
            }

            if size <= limit {
                package.blocks.push((block.header, txn_indicies));
                package.txns.append(new_txns);
            } else { break; }
        }

        Ok(package)
    }

    /// Convert the `BlockPackage` into a compressed binary representation which can be easily
    /// transferred or archived.
    pub fn pack(&self) -> Vec<u8> {
        let raw = bincode::serilize(self, bincode::Infinite).unwrap();
        compress(&raw).unwrap()
    }

    /// Unpack a compressed block binary representation of the `BlockPackage`.
    pub fn unpack(package: &[u8]) -> BlockPackage {
        let raw = decompress(package).unwrap();
        bincode::deserialize(&raw)
    }
}