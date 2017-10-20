use std::collections::LinkedList;

use blockscape_core::record_keeper::database::MutationRules;

pub fn build_rules() -> MutationRules {
    LinkedList::new()
}