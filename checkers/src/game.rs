use checkers;
use std::sync::Arc;
use parking_lot::RwLock;
use blockscape_core::record_keeper::{RecordKeeper, GameStateCache, Error, PlotID, PlotEvent};
use blockscape_core::primitives::{Txn, Mutation, Change};
use std::collections::BTreeSet;
use blockscape_core::bin::*;
use blockscape_core::time::Time;
use blockscape_core::hash::hash_pub_key;
use openssl::pkey::PKey;
use bincode;

pub type GameCache = Arc<RwLock<GameStateCache<checkers::Board>>>;


#[inline]
pub fn create_cache() -> GameCache {
    Arc::new(RwLock::new(GameStateCache::new()))
}


pub struct CheckersGame {
    pub rk: Arc<RecordKeeper>,
    pub sign_key: PKey,
    pub cache: GameCache
}

impl CheckersGame {
    /// Get the game board at the given plot. If tick is specified, it will attempt to get a board
    /// at that tick, if the tick comes after the latest known information, then it will simply give
    /// the latest board. If it is None, it will simply get the latest known state. If no game has
    /// been started on the given plot, it will return the default starting board.
    pub fn get_board(&self, location: PlotID, tick: Option<u64>) -> Result<checkers::Board, Error> {
        let (actual_tick, mut board) = self.cache.read().latest(location, tick)
            .map(|(t, b)| (t, b.clone()))
            .unwrap_or((0, checkers::Board::default()));
        debug_assert!(!tick.is_some() || actual_tick <= tick.unwrap());

        if !tick.is_some() || actual_tick < tick.unwrap() { return Ok(board); }

        // update state
        let tick = tick.unwrap();
        let raw_events = self.rk.get_plot_events(location, 0)?;
        for (actual_tick, raw_event_list) in raw_events {
            if actual_tick == 0 { continue; }
            else if actual_tick > tick { break; }
            debug_assert_eq!(raw_event_list.len(), 1);
            board.play(
                bincode::deserialize(&raw_event_list[0])?,
                checkers::Player::from_turn(actual_tick).unwrap()
            ).unwrap();
        } Ok(board)
    }

    /// Will return the set of all actions on a game board that are known (sorted by ascending
    /// tick). Will return an empty list if no game has been started on the given plot.
    pub fn get_moves(&self, location: PlotID) -> Result<Vec<checkers::Event>, Error> {
        let raw_events = self.rk.get_plot_events(location, 0)?;

        let mut events = Vec::new();
        for (_tick, raw_event_list) in raw_events {
            debug_assert_eq!(raw_event_list.len(), 1);
            events.push(bincode::deserialize(&raw_event_list[0])?);
        } Ok(events)
    }

    /// Wrap a checkers event in a txn and submit it to record keeper.
    pub fn play(&self, location: PlotID, event: checkers::Event) -> Result<(), Error> {
        let tick = self.get_moves(location)?.len() as u64;
        debug!("Playing {:?} on turn {}", event, tick);
        let change = Change::PlotEvent(PlotEvent{
            from: location,
            to: BTreeSet::new(),
            tick,
            event: event.as_bin()
        });

        let mutation = {
            let mut m = Mutation::new();
            m.changes.push(change); m
        };

        let txn = Txn {
            timestamp: Time::current(),
            creator: hash_pub_key(&self.sign_key.public_key_to_der().unwrap()),
            mutation,
            signature: Bin::new()
        }.sign(&self.sign_key);

        self.rk.add_pending_txn(txn, true)?;
        Ok(())
    }
}