use std::collections::LinkedList;

// Will contain multiple changes and will also have data needed to verify a mutation.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Mutation;

impl Mutation {
    pub fn merge(mutations: LinkedList<Mutation>) -> Mutation {
        unimplemented!()
    }
}