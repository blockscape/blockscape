use bin::*;
use bincode;
use hash::hash_obj;
use openssl::pkey::PKey;
use primitives::{Mutation, JMutation, U256, U160, JU160};
use signer::{sign_bytes, verify_bytes};
use std::cmp::Ordering;
use std::mem::size_of;
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
    pub creator: U160,
    pub mutation: Mutation,
    pub signature: Bin,
}


impl PartialEq for Txn {
    fn eq(&self, other: &Txn) -> bool {
        self.calculate_hash() == other.calculate_hash()
    }
} impl Eq for Txn {}

impl PartialOrd for Txn {
    fn partial_cmp(&self, other: &Txn) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Txn {
    fn cmp(&self, other: &Txn) -> Ordering {
        let a = self.calculate_hash();
        let b = other.calculate_hash();
        a.cmp(&b)
    }
}


impl Txn {
    pub fn new(creator: U160, mutation: Mutation) -> Txn {
        Txn {
            timestamp: Time::current(),
            creator,
            mutation,
            signature: Bin::new()
        }
    }

    pub fn calculate_hash(&self) -> U256 {
        hash_obj(self)
    }

    pub fn sign(self, key: &PKey) -> Txn {
        let bytes = self.get_signing_bytes();
        Txn {
            timestamp: self.timestamp,
            creator: self.creator,
            mutation: self.mutation,
            signature: sign_bytes(&bytes, key).into()
        }
    }

    pub fn verify_signature(&self, key: &PKey) -> bool {
        let bytes = self.get_signing_bytes();
        verify_bytes(&bytes, &self.signature, key)
    }

    fn get_signing_bytes(&self) -> Bin {
        let mut bytes = Bin::new();
        bytes.extend(bincode::serialize(&self.timestamp, bincode::Bounded(8)).unwrap());
        bytes.extend(self.creator.to_vec());
        bytes.extend(bincode::serialize(&self.mutation, bincode::Infinite).unwrap());
        bytes
    }

    /// Calculate the encoded size of this transaction in bytes.
    pub fn calculate_size(&self) -> usize {
        size_of::<Time>() +  // timestamp
        size_of::<U160>() + //creator
        self.mutation.calculate_size() +
        (self.signature.len() + 1)
    }
}

    

    // /// Span a new parent shard. Create a transaction which adds a new head to the tree. One of the
    // /// new shard's children will be the current head of the tree, and it will add more shards
    // /// beyond that.
    // /// TODO: create needed mutations
    // pub fn new_expanding_txn(pkey: &PKey) -> Txn {
    //     let mutation = Mutation::new();
    //     Self::new_txn(EXPANDING_TXN, pkey, mutation)
    // }

    // /// A transaction which divides a shard into multiple new shards breaking up the work required
    // /// to compute it. The shard with this transaction will be turned into a parent shard.
    // /// TODO: create needed mutations.
    // pub fn new_split_txn(pkey: &PKey) -> Txn {
    //     let mutation = Mutation::new();
    //     Self::new_txn(SPLIT_TXN, pkey, mutation)
    // }

    // /// A transaction which adds a new validator (or player) to the network. This type of
    // /// transaction must be signed by the admin key to be valid to prevent people from creating many
    // /// new accounts to gain reputation faster.
    // /// TODO: create needed mutations
    // pub fn new_validator_txn(pkey: &PKey) -> Txn {
    //     let mutation = Mutation::new();
    //     Self::new_txn(ADD_VALIDATOR_TXN, pkey, mutation)
    // }

    // /// Update the reference to a child-shard's current block in the chain. This should only happen
    // /// if the reference has not been stored before (aka avoid duplicate references).
    // /// TODO: create needed mutations
    // pub fn new_child_block_ref_txn(pkey: &PKey) -> Txn {
    //     let mutation = Mutation::new();
    //     Self::new_txn(CHILD_BLOCK_REF_TXN, pkey, mutation)
    // }

    // /// Create a transaction which indicates a mutation to two different shards. This information
    // /// will be propagated up and down the shard tree.
    // /// TODO: create needed mutations
    // pub fn new_shard_transfer_txn(pkey: &PKey) -> Txn {
    //     // Will need a `from` and `to` shard along with the changes.
    //     let mutation = Mutation::new();
    //     Self::new_txn(SHARD_TRANSFER_TXN, pkey, mutation)
    // }

    // /// Create a transaction to reward those who voted for only this block and chastise those who
    // /// voted for a competing block. Note that those who vote for more than one block,    pub data: Vec<u8>, a slashing
    // /// transaction should instead be made and they should not be included in a ballot txn.
    // /// TODO: create needed mutations
    // pub fn new_ballot_txn(pkey: &PKey) -> Txn {
    //     // (Can only reward/punish for votes on whether this block should be accepted)
    //     // List of nodes who backed correct chain:
    //     //      Who it wasbroken into more than one creation function depending on the
    //     // change that is to be made.
    //     //      (I do not believe we need to keep evidence given the honest majority assumption,
    //     //       will need to make sure that the proof of a positive vote can be requested in
    //     //       case they choose to selectively send their votes to different nodes)
    //     // List of nodes who backed incorrect chain:
    //     //      Who it was
    //     //      Record of their action
    //     //      Their signature for the action
    //     let mutation = Mutation::new();
    //     Self::new_txn(BALLOT_TXN, pkey, mutation)
    // }

    // /// Create a transaction designed to significantly hurt the reputation of someone who acted can
    // /// be proven to have acted invalidly. A perfect example of this is voting on two competing
    // /// blocks.
    // /// TODO: create needed mutations
    // pub fn new_slash_txn(pkey: &PKey) -> Txn {
    //     // list of:
    //     //      misbehaving node
    //     //      evidence in the form of their signed actions
    //     let mutation = Mutation::new();
    //     Self::new_txn(SLASH_TXN, pkey, mutation)
    // }

    // /// Create a new generic data blob which can store things like network state or game state.
    // /// These should only occur ever so many blocks and will allow people to quickly get updated in
    // /// favor of recalculating state from genesis.
    // /// TODO: create needed mutations
    // pub fn new_state_txn(pkey: &PKey) -> Txn {
    //     let mutation = Mutation::new();
    //     Self::new_txn(STATE_TXN, pkey, mutation)
    // }

    // /// Create a new transaction which has mostly unchecked power. The primary requirement is that
    // /// the admin key must sign off on it. This will make debugging easier and allow us to correct
    // /// issues as they come up.
    // pub fn new_admin_txn(pkey: &PKey) -> Txn {
    //     // Seems like this might get broken into more than one creation function depending on the
    //     // change that is to be made.
    //     let mutation = Mutation::new();
    //     Self::new_txn(ADMIN_TXN, pkey, mutation)
    // }
// }



#[derive(Serialize, Deserialize)]
pub struct JTxn {
    pub timestamp: Time,
    pub creator: JU160,
    pub mutation: JMutation,
    pub signature: JBin,
}

impl From<Txn> for JTxn {
    fn from(t: Txn) -> JTxn {
        JTxn {
            timestamp: t.timestamp,
            creator: t.creator.into(),
            mutation: t.mutation.into(),
            signature: t.signature.into()
        }
    }
}

impl Into<Txn> for JTxn {
    fn into(self) -> Txn {
        Txn {
            timestamp: self.timestamp,
            creator: self.creator.into(),
            mutation: self.mutation.into(),
            signature: self.signature.into()
        }
    }
}