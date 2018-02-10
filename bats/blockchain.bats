#!/usr/bin/env bats
load 'helpers'

setup() {
    clean_test_cluster || true
    stop_blockscape_server || true
}

@test "gets_current_block_information" {
    run_test_cluster
    start_blockscape_server
    wait_for_rpc

    blockscape get_current_block_hash
    blockscape get_current_block_header
    blockscape get_current_block

    stop_blockscape_server
}

@test "adds_blocks_and_txns" {
    run_test_cluster
    start_blockscape_server
    wait_for_rpc

    blockscape sign_txn '[{ "timestamp": 151822146000, "creator": "0", "mutation": { "contra": false, "changes": [] }, "signature": "0" }]'

    stop_blockscape_server
}