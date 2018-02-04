use blockscape_core::record_keeper::MutationRules;

mod valid_event;
mod turns;

pub fn build_rules() -> MutationRules {
    let mut rules = MutationRules::new();
    rules.push_back(Box::new(valid_event::ValidEvent));
    rules.push_back(Box::new(turns::Turns));
    rules
}