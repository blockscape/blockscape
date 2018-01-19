#!/bin/bash

if [ $# -gt 0 ]; then
	docker run -t --rm -e RUST_LOG=debug --net host dcr.buyme360.com/plu-capstone/blockscape:latest /blockscape "$@"
else
	docker run -t --rm -e RUST_LOG=debug --net host dcr.buyme360.com/plu-capstone/blockscape:latest
fi
