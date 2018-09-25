use std::cell::Cell;
use std::rc::Rc;

use primitives::Block;

use network::protocol::*;
use network::context::NetworkContext;

use primitives::U256;

thread_local!(static ACTIVE_SYNCS: Cell<usize> = Cell::new(0));

/// A data retrieval task assigned to a specific client
#[derive(Debug)]
pub struct NetworkJob {
    /// The number of times this job has failed to resolve
    pub try: Cell<usize>,

    /// The job to process after accomplishing this job
    data: NetworkJobData
}

/// The actual payload defining the data retrieval which should occur
#[derive(Debug, Clone)]
pub enum NetworkJobData {
    /// Synchronize the chain to the given block (picked up by the network prior). 
    /// The second constant indicates the predicted blocks downloaded (but not necessarily imported) so far
    SyncChain(Block, U256),

    /// Get a list of nodes from the remote peer for the given network id in order to grow our contacts
    FindNodes(U256)
}

impl NetworkJob {

    pub fn new(data: NetworkJobData) -> NetworkJob {

        if let NetworkJobData::SyncChain(..) = data {
            ACTIVE_SYNCS.with(|v| v.set(v.get() + 1));
        }
        
        NetworkJob {
            try: Cell::new(0),
            data: data
        }
    }

    pub fn chain_sync_exists() -> bool {

        let mut r = false;

        ACTIVE_SYNCS.with(|v| {
            r = v.get() > 0;
        });

        r
    }

    pub fn make_req(&self, _ctx: &Rc<NetworkContext>) -> Message {
        match &self.data {
            &NetworkJobData::FindNodes(ref network_id) => Message::FindNodes {
                network_id: network_id.clone(),
                skip: 0
            },
            &NetworkJobData::SyncChain(ref cblock, ref last) => Message::SyncBlocks {
                last_block_hash: last.clone(),
                target_block_hash: cblock.calculate_hash()
            }
        }
    }

    /// Checks to see if the current job can be updated with new data from a new job.
    /// Returns true if the provided network job should be considered a duplicate after a possible augmentation operation
    pub fn augment(&mut self, other: &NetworkJob) -> bool {
        match &mut self.data {
            NetworkJobData::SyncChain(target, _cur) => {
                if let &NetworkJobData::SyncChain(ref otarget, ref _ocur) = &other.data {
                    let my_hash = target.calculate_hash();
                    if my_hash == otarget.calculate_hash() {
                        return true;
                    }

                    if my_hash == otarget.prev {
                        *target = otarget.clone();
                        return true;
                    }
                }
            },

            _ => {}
        }

        false
    }

    /// Called when the job has been completed and processed
    pub fn complete(self, msg: &Message, ctx: &Rc<NetworkContext>) -> Option<NetworkJob> {
        
        match self.data {
            NetworkJobData::SyncChain(ref target, mut cur) => {

                let mut try = self.try.get();

                // augment and return this job if we have not reached the target
                if let &Message::ChainData(ref hash, ..) = msg {

                    if hash == &target.calculate_hash() {
                        return None; // chain should be completely synced up
                    }

                    cur = *hash;
                }
                else if let &Message::DataError(ref err) = msg {
                    if let &DataRequestError::HashesNotFound(..) = err {
                        // TODO: This seems like a good thing to act on
                    }

                    try = try + 1;
                }
                else {
                    // packet does not correspond to what we requested of the client
                    warn!("Invalid response for SyncChain data: {:?}", msg);
                    try = try + 1;
                }

                if try > MAX_JOB_RETRIES {
                    warn!("Dropping job: {:?}", self);

                    return None;
                }

                let mut nj = NetworkJob::new(NetworkJobData::SyncChain(target.clone(), cur));

                nj.try.set(try);

                Some(nj)
            },

            NetworkJobData::FindNodes(network_id) => {

                if let &Message::NodeList { ref nodes, .. } = msg {
                    if let Some(ref shard) = *ctx.get_shard_by_id(&network_id) {
                        // immediately try to open connections
                        // limit results we use to the maximum number of new connections
                        for node in nodes.iter().take(ctx.config.max_nodes as usize - ctx.config.min_nodes as usize) {
                            if let Err(e) = shard.open_session(node.clone(), None, true) {
								debug!("Failed to open session (find nodes job): {:?}", e);
							}
                        }
                    }
                }

                None
            },

            //_ => None
        }
    }
}

impl Clone for NetworkJob {
    fn clone(&self) -> NetworkJob {
        if let NetworkJobData::SyncChain(..) = self.data {
            ACTIVE_SYNCS.with(|v| v.set(v.get() + 1));
        }
        
        NetworkJob {
            try: self.try.clone(),
            data: self.data.clone()
        }
    }
}

impl Drop for NetworkJob {
    fn drop(&mut self) {
        if let NetworkJobData::SyncChain(..) = self.data {
            ACTIVE_SYNCS.with(|v| v.set(v.get() - 1));
        }
    }
}
