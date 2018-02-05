use std::collections::{HashMap, BTreeMap};
use super::PlotID;

/// A cache of plot game states by tick
pub struct GameStateCache<S> {
    states: BTreeMap<u64, HashMap<PlotID, S>>,
    latest: HashMap<PlotID, u64>
}

impl<S> GameStateCache<S> {
    /// Cache a state. Will remove any cached states for that plot which come at a later tick.
    pub fn cache(&mut self, tick: u64, plot: PlotID, state: S) {
        if !self.states.contains_key(&tick) {
            self.states.insert(tick, HashMap::new());
        }
        self.states.get_mut(&tick).unwrap().insert(plot, state);

        if //we need to drop elements after...
            if let Some(l) = self.latest.get(&plot) {
                if *l > tick {true}
                else {false}
            } else {false}
        { self.drop_after(tick, Some(plot)); }

        // update the latest
        self.latest.insert(plot, tick);
    }

    /// Returns the latest information for a plot and the tick it is. If a tick is supplied, then it
    /// returns the latest cache which is before or at the specified tick.
    pub fn latest(&self, plot: PlotID, tick: Option<u64>) -> Option<(u64, &S)> {
        let latest = self.latest.get(&plot);
        if latest.is_none() {return None;}
        let latest = *latest.unwrap();

        if tick.is_none() || latest <= tick.unwrap() {
            Some( (latest, self.states.get(&latest).unwrap().get(&plot).unwrap()) )
        } else {
            let tick = tick.unwrap();
            self.states.iter().rev()
                .skip_while(|&(t, _)| *t > tick)
                .filter(|&(_, s)| s.contains_key(&plot))
                .next()
                .map(|(t, s)| (*t, s.get(&plot).unwrap()) )
        }
    }

    /// Drop any cached data before a certain point. If Plot is specified it will only drop things
    /// before the tick for a certain plot.
    pub fn drop_before(&mut self, tick: u64, plot: Option<PlotID>) {
        if let Some(plot) = plot {
            for (t, plots) in self.states.iter_mut() {
                if *t >= tick { break; }
                plots.remove(&plot);
            }
        } else {
            self.states = self.states.split_off(&tick);
        }
    }

    /// Drop all cached information for a given plot
    pub fn drop_plot(&mut self, plot: PlotID) {
        for (_, plots) in self.states.iter_mut() {
            plots.remove(&plot);
        }
    }

    /// Drop any cached data after a certain point. If Plot is specified it will only drop things
    /// after the tick for a certain plot.
    fn drop_after(&mut self, tick: u64, plot: Option<PlotID>) {
        if let Some(plot) = plot {
            for (t, plots) in self.states.iter_mut().rev() {
                if *t <= tick { break; }
                plots.remove(&plot);
            }
        } else {
            let t = self.states.remove(&tick);
            self.states.split_off(&tick);
            if t.is_some() { self.states.insert(tick, t.unwrap()); }
        }
    }
}