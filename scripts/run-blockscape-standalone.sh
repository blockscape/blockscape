#!/bin/bash

if [ $# -gt 0 ]; then
	docker run -it --rm -e RUST_LOG=blockscape=debug,blockscape_core=debug,info --net host dcr.buyme360.com/plu-capstone/blockscape:latest /blockscape --rpcbind 127.0.0.1 "$@"
else
	docker run -it --rm -e RUST_LOG=blockscape=debug,blockscape_core=debug,info --net host dcr.buyme360.com/plu-capstone/blockscape:latest
fi
