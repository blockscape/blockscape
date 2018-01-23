# Returns the directory of the test scripts
test_dir() {
    SOURCE="${BASH_SOURCE[0]}"

    echo "$(dirname $SOURCE)"
}

start_blockscape_server() {
    "$(test_dir)/../scripts/start-blockscape-standalone.sh" "$@"
}

stop_blockscape_server() {
    "$(test_dir)/../scripts/stop-blockscape-standalone.sh" "$@"
}

# Returns the blockscape executable
blockscape() {
    "$(test_dir)/../target/debug/blockscape" "$@"

    return $!
}

# Starts the official blockscape testing cluster using docker-compose
run_test_cluster() {
    docker-compose -f "$(test_dir)/../scripts/docker-compose.yml" up -d
}

# Shuts down a testing cluster previously spun up with a call to run_test_cluster
clean_test_cluster() {
    docker-compose -f "$(test_dir)/../scripts/docker-compose.yml" down
}

# Continuously check for when the RPC becomes available and return when that happens
wait_for_rpc() {
    echo "Waiting for RPC..."

    for i in {0..150}; do

        if blockscape get_net_stats > /dev/null; then
            return 0 # ready to go
        fi

        sleep 0.1

    done

    echo "Failed to wait for RPC!"

    return 1
}


# Continuously run the command for a certain number of seconds, failing if it never succeeds
wait_for() {

    n=$1

    shift

    for i in $(seq 1 $n); do

        echo "$i Run in wait_for $@"

        set +e
        TEST=$(eval "$@" > /dev/null)$?
        blockscape get_net_stats
        echo $TEST
        set -e

        echo "Test done"

        if [ $TEST -eq 0 ]; then
            echo "$@"
            return 0
        fi

        sleep 1
    done

    echo "Wait failed."

    return 1 # technically we just failed to wait
}