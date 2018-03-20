use bin::Bin;
use primitives::{Change, U160};
use record_keeper::{Error, LogicError, NetState};
use record_keeper::rules::MutationRule;
use std::collections::HashMap;
use record_keeper::key::NetworkEntry;

/// Make sure there are enough shares for transfers and slash txns. Also make sure
pub struct Shares;
impl MutationRule for Shares {
    fn is_valid(&self, state: &NetState, mutation: &Vec<(Change, U160)>, _cache: &mut Bin) -> Result<(), Error> {
        // record the people sending money to make sure they do not send more than they have.
        let mut senders: HashMap<U160, u64> = HashMap::new();

        for &(ref change, creator) in mutation {
            if let &Change::Transfer{from, ref to} = change {
                if from != creator && from != state.get_obj(NetworkEntry::AdminKeyID.into())? {
                    // must be created by the sender or by the admin.
                    return Err(LogicError::InvalidSigner.into())
                }

                let subtotal = to.iter()
                    .fold(0u64, |acc, (_, &v)| acc + v);
                let prior_balance = senders.get(&from).cloned().unwrap_or(0u64);
                let new_subtotal = subtotal.checked_add(prior_balance)
                    .ok_or(LogicError::InvalidMutation("Overflowing addition".into()))?;
                senders.insert(from, new_subtotal);
            }
        }

        for (sender, amount) in senders {
            let stake = state.get_validator_stake(sender)?;
            if stake < amount {
                // cannot send more shares than the sender has.
                return Err(LogicError::NotEnoughShares.into())
            }
            if amount > <i64>::max_value() as u64 {
                // catch an overflow here because we use i64 arithmetic to change the values internally.
                return Err(LogicError::InvalidMutation("Possible overflow due to large value.".into()).into())
            }
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Share calculations must leave accounts with a valid number of shares and must be sent by the owner."
    }
}