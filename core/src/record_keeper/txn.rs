use openssl::pkey::PKey;
use signer::sign_obj;
use std::vec::Vec;
use super::mutation::Mutation;
use time::Time;

// Expand and divide shard transactions
// Positive reputation transaction ?
// Slashing transaction
// Superblocks store hashes of sublocks current subchain blocks
// Any mutations to game state
// Transfers between shards must be signed by a bunch of people

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Represents a Transaction on the network.
pub struct Txn {
    pub timestamp: Time,
    pub txn_type: u8,
    pub pubkey: Vec<u8>,  // stored as a DER format
    pub mutation: Mutation,
    pub signature: Vec<u8>,
}

pub const EXPANDING_TXN: u8 = 0;
pub const SPLIT_TXN: u8 = 1;
pub const ADD_VALIDATOR_TXN: u8 = 2;
pub const CHILD_BLOCK_REF_TXN: u8 = 3;
pub const SHARD_TRANSFER_TXN: u8 = 4;
pub const BALLOT_TXN: u8 = 5;
pub const SLASH_TXN: u8 = 6;
pub const STATE_TXN: u8 = 7;
pub const ADMIN_TXN: u8 = 9;



impl Txn {
    /// Assume default values for a number of features and manually set the data, mutations, and
    /// transaction type. Everything else is Either a default or derived.
    fn new_txn(txn_type: u8, pkey: &PKey, mutation: Mutation) -> Txn {
        let signature = sign_obj(&mutation, pkey);

        Txn {
            timestamp: Time::current(),
            txn_type,
            pubkey: pkey.public_key_to_der().unwrap(),
            mutation, signature
        }
    }

    /// Span a new parent shard. Create a transaction which adds a new head to the tree. One of the
    /// new shard's children will be the current head of the tree, and it will add more shards
    /// beyond that.
    /// TODO: create needed mutations
    pub fn new_expanding_txn(pkey: &PKey) -> Txn {
        let mutation = Mutation::new();
        Self::new_txn(EXPANDING_TXN, pkey, mutation)
    }

    /// A transaction which divides a shard into multiple new shards breaking up the work required
    /// to compute it. The shard with this transaction will be turned into a parent shard.
    /// TODO: create needed mutations.
    pub fn new_split_txn(pkey: &PKey) -> Txn {
        let mutation = Mutation::new();
        Self::new_txn(SPLIT_TXN, pkey, mutation)
    }

    /// A transaction which adds a new validator (or player) to the network. This type of
    /// transaction must be signed by the admin key to be valid to prevent people from creating many
    /// new accounts to gain reputation faster.
    /// TODO: create needed mutations
    pub fn new_validator_txn(pkey: &PKey) -> Txn {
        let mutation = Mutation::new();
        Self::new_txn(ADD_VALIDATOR_TXN, pkey, mutation)
    }

    /// Update the reference to a child-shard's current block in the chain. This should only happen
    /// if the reference has not been stored before (aka avoid duplicate references).
    /// TODO: create needed mutations
    pub fn new_child_block_ref_txn(pkey: &PKey) -> Txn {
        let mutation = Mutation::new();
        Self::new_txn(CHILD_BLOCK_REF_TXN, pkey, mutation)
    }

    /// Create a transaction which indicates a mutation to two different shards. This information
    /// will be propagated up and down the shard tree.
    /// TODO: create needed mutations
    pub fn new_shard_transfer_txn(pkey: &PKey) -> Txn {
        // Will need a `from` and `to` shard along with the changes.
        let mutation = Mutation::new();
        Self::new_txn(SHARD_TRANSFER_TXN, pkey, mutation)
    }

    /// Create a transaction to reward those who voted for only this block and chastise those who
    /// voted for a competing block. Note that those who vote for more than one block,    pub data: Vec<u8>, a slashing
    /// transaction should instead be made and they should not be included in a ballot txn.
    /// TODO: create needed mutations
    pub fn new_ballot_txn(pkey: &PKey) -> Txn {
        // (Can only reward/punish for votes on whether this block should be accepted)
        // List of nodes who backed correct chain:
        //      Who it wasbroken into more than one creation function depending on the
        // change that is to be made.
        //      (I do not believe we need to keep evidence given the honest majority assumption,
        //       will need to make sure that the proof of a positive vote can be requested in
        //       case they choose to selectively send their votes to different nodes)
        // List of nodes who backed incorrect chain:
        //      Who it was
        //      Record of their action
        //      Their signature for the action
        let mutation = Mutation::new();
        Self::new_txn(BALLOT_TXN, pkey, mutation)
    }

    /// Create a transaction designed to significantly hurt the reputation of someone who acted can
    /// be proven to have acted invalidly. A perfect example of this is voting on two competing
    /// blocks.
    /// TODO: create needed mutations
    pub fn new_slash_txn(pkey: &PKey) -> Txn {
        // list of:
        //      misbehaving node
        //      evidence in the form of their signed actions
        let mutation = Mutation::new();
        Self::new_txn(SLASH_TXN, pkey, mutation)
    }

    /// Create a new generic data blob which can store things like network state or game state.
    /// These should only occur ever so many blocks and will allow people to quickly get updated in
    /// favor of recalculating state from genesis.
    /// TODO: create needed mutations
    pub fn new_state_txn(pkey: &PKey) -> Txn {
        let mutation = Mutation::new();
        Self::new_txn(SLASH_TXN, pkey, mutation)
    }

    /// Create a new transaction which has mostly unchecked power. The primary requirement is that
    /// the admin key must sign off on it. This will make debugging easier and allow us to correct
    /// issues as they come up.
    pub fn new_admin_txn(pkey: &PKey) -> Txn {
        // Seems like this might get broken into more than one creation function depending on the
        // change that is to be made.
        unimplemented!("Admin transactions have not been implemented");
    }
}