#!/usr/bin/env bats

load "helpers"

setup() {
    clean_test_cluster || true
    stop_blockscape_server || true
}

@test "connects to given seed node" {
    run_test_cluster

    start_blockscape_server

    wait_for_rpc
    wait_for 15 '[ "$(blockscape get_net_stats | jq .connected_peers)" -ge 1 ]'

    #docker ps

    #blockscape get_net_stats | jq '.connected_peers'

    #false

    stop_blockscape_server
}

@test "connects to given seed and other nodes" {

    skip

    run_test_cluster

    blockscape &

    [ $($(wait_for 180 blockscape get_net_stats | jq '.connected_peers') -gte 3) ]

    blockscape stop

}