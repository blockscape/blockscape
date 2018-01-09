use bincode;
use compress::{compress, decompress};
use primitives::{Block, BlockHeader, Txn, U256};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::mem::size_of;
use super::database::Database;
use super::Error;

/// Estimated block size in bytes, this should be slightly under the true value, and will be used 
const ESTIMATED_BLOCK_SIZE: usize = 128;

/// A self-contained package of blocks and their associated transactions designed for easy transfer
/// between systems. This takes in reference to the database for construction to prevent other code
/// form being responsible for an invalid ordering of blocks or the txn references internally
/// stored.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlockPackage {
    /// Blocks and their associated transactions sorted from the lowest height to the greatest height.
    blocks: Vec<(BlockHeader, Vec<u16>)>,
    /// The txns which are referenced by at least one of the included blocks.
    txns: Vec<Txn>
}

impl BlockPackage {
    /// Create a new, empty blockpackage.
    fn new() -> BlockPackage {
        BlockPackage { blocks: Vec::new(), txns: Vec::new() }
    }

    /// Create a `BlockPackage` of blocks before the `target` hash until it collides with the main
    /// chain. If the `start` hash lies between the target and the main chain, it will return the
    /// blocks between them, otherwise it will return the blocks from the main chain until target
    /// in that order and it will not include the start or target blocks.
    ///
    /// If the limit is reached, it will prioritize blocks of a lower height, but may have a gap
    /// between the main chain (or start) and what it includes.
    pub fn blocks_before(db: &Database, last_known: &U256, target: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let blocks = db.get_blocks_before(last_known, target, limit / ESTIMATED_BLOCK_SIZE)?;
        Self::package(db, blocks, limit)
    }

    /// Create a `BlockPackage` of all the blocks of the current chain which are a descendent of
    /// the latest common ancestor between the chain of the start block and the current chain. This
    /// result will be sorted in ascending height order. It will not include the start hash. Also,
    /// `limit` is the maximum number of blocks it should scan through when ascending the
    /// blockchain.
    pub fn blocks_after(db: &Database, start: &U256, limit: usize) -> Result<BlockPackage, Error> {
        let blocks = db.get_blocks_after(start, limit / ESTIMATED_BLOCK_SIZE)?;
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
        for block in blocks.iter() {
            for txn in block.txns.iter() {
                if !txns_by_hash.contains_key(&txn) {
                    txns_by_hash.insert(*txn, count);
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
            size += size_of::<BlockHeader>() + (block.txns.len() + 1) * 2;

            for txn in block.txns {
                let index = *txns_by_hash.get(&txn).unwrap();
                txn_indicies.push(index);
                
                if index == count { //we need to add the txn itself
                    let full_txn = db.get_txn(&txn)?;
                    size += full_txn.calculate_size();
                    new_txns.push(full_txn);
                    count += 1;
                }
            }

            if size <= limit {
                package.blocks.push((block.header, txn_indicies));
                package.txns.append(&mut new_txns);
            } else { break; }
        }

        Ok(package)
    }

    /// Convert the `BlockPackage` into a compressed binary representation which can be easily
    /// transferred or archived.
    pub fn zip(&self) -> Result<Vec<u8>, Error> {
        let raw = bincode::serialize(self, bincode::Infinite).map_err(|_| Error::Deserialize("".into()))?;
        compress(&raw).map_err(|_| Error::Deserialize("".into()))
    }

    /// Unpack a compressed block binary representation of the `BlockPackage`.
    pub fn unzip(package: &[u8]) -> Result<(BlockPackage, usize), Error> {
        let raw = decompress(package).map_err(|_| Error::Deserialize("".into()))?;
        let s = raw.len();
        bincode::deserialize(&raw)
            .map(|r| (r, s))
            .map_err(|_| Error::Deserialize("".into()))
    }

    /// Unpacks the information within into a more useful form.
    pub fn unpack(self) -> (Vec<Block>, HashMap<U256, Txn>) {
        let txns = self.txns.into_iter()
            .map(|txn| (txn.calculate_hash(), txn))
            .collect::<Vec<(U256, Txn)>>();
    
        let blocks = self.blocks.into_iter()
            .map(|(header, txn_list)| {
                let txn_list = txn_list.into_iter()
                    .filter_map(|txn_id| txns.get(txn_id as usize))
                    .map(|t| t.0 )
                    .collect::<BTreeSet<U256>>();
                
                Block{header, txns: txn_list}
            }).collect::<Vec<Block>>();
        
        (blocks, txns.into_iter().collect())
    }

    /// Get the last block hash serviced by this block package
    pub fn last_hash(&self) -> U256 {
        self.blocks.last().unwrap().0.calculate_hash()
    }

    /// Returns the hash prior to the first block serviced by this block package
    pub fn starts_at(&self) -> U256 {
        self.blocks.first().unwrap().0.prev
    }
}