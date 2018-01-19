docker run --privileged -t --rm --net host -e RUST_LOG=blockscape_core=debug -p 2345:2345 dcr.buyme360.com/plu-capstone/blockscape:latest gdbserver 127.0.0.1:2345 /blockscape "$@"
