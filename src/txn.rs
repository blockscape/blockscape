use std::vec::Vec;
use u256::U256;

// Expand and divide shard transactions
// Positive reputation transaction ?
// Slashing transaction
// Superblocks store hashes of sublocks current subchain blocks
// Any mutations to game state
// Transfers between shards must be signed by a bunch of people

#[derive(Debug, Serialize, Deserialize)]
/// Represents a Transaction on the network.
pub struct Txn {
    pub timestamp: u64,
    pub txn_type: u8,
    pub pubkey: U256,
    pub mutations: Vec<u8>,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}

// #[derive(Debug)]
// pub enum TxnData {
//     Variant1,
//     Variant2,
// }

/// A transaction which adds a new head to the tree and new children,
/// (the current head will become a child of the new head).
trait ExpandingTxn {
    // Spawned Parent Shard
}

/// A transaction which divides a shard into multiple new shards to
/// divide up the work effort. The shard with this transaction will
/// be turned into a parent shard.
trait SplitTxn {
    // List of new children shards
}

/// A transaction which adds a new validator. They will need to include
/// signed approval from the admin key.
trait AddValidatorTxn {
    // Signed (by admins) hash of public key
}

/// A transaction to point to the current block in a child shard of
/// this shard.
trait ChildBlockRefTxn {
    // Child block
}

/// A transaction which indicates a mutation to two different shards.
/// Will be propagated up and down a tree.
trait ShardTransferTxn {
    // From shard
    // To shard
    // Modifications
}

/// A transaction which rewards those who voted for only this block and chastises
/// those who voted for a competing block.
trait BallotTxn {
    // (Can only reward/punish for votes on whether this block should be accepted)
    // List of nodes who backed correct chain:
    //      Who it was
    //      (I do not believe we need to keep evidence given the honest majority assumption,
    //       will need to make sure that the proof of a positive vote can be requested in
    //       case they choose to selectively send their votes to different nodes)
    // List of nodes who backed incorrect chain:
    //      Who it was
    //      Record of their action
    //      Their signature for the action
}

/// A transaction which significantly punishes an individual for demonstratable misbehavior.
trait SlashTxn {
    // list of:
    //      misbehaving node
    //      evidence in the form of their signed actions
}

trait StateTxn {
    // generic data blob that does stuff to game state
}

/// A transaction which mutates the state and has no validity checks.
/// This should be removed at some point, note that the signature must
/// be from the correct key for this to be valid.
trait AdminStateTxn {
    // generic data blob that does stuff to game state
}

impl Txn {
    // To txntype
}