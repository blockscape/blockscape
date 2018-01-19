#!/bin/bash

if [ $# -gt 0 ]; then
	docker run -t --rm --net host -e RUST_LOG=blockscape_core=debug dcr.buyme360.com/plu-capstone/blockscape:latest /blockscape "$@"
else
	docker run -t --rm --net host -e RUST_LOG=blockscape_core=debug dcr.buyme360.com/plu-capstone/blockscape:latest
fi
