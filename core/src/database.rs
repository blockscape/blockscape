use rocksdb::DB;
use rocksdb::Error as DBError;
use mutation::Mutation;
use std::collections::LinkedList;
use block::Block;
use txn::Txn;

trait MutationRule {
    fn is_valid(&self, mutation: &Mutation, database: &DB) -> bool;
}

pub struct Database {
    db: DB,
    rules: LinkedList<Box<MutationRule>>,
}

impl Database {
    // pub fn add_rule(&self, rule: Box<MutationRule>) {

    // }

    // pub fn is_valid(mutation: Mutation) -> Result((), String) {

    // }

    // pub fn mutate(mutation: Mutation) -> Result((), String) {

    // }

    // pub fn get_block(hash: U256) -> Option<Block> {

    // }

    // pub fn get_txn(hash: U256) -> Option<Txn> {

    // }
}