#!/bin/bash

if [ $# -gt 0 ]; then
	docker run --name blockscape -it --rm dcr.buyme360.com/plu-capstone/blockscape:latest /blockscape "$@"
else
	docker run --name blockscape -it --rm dcr.buyme360.com/plu-capstone/blockscape:latest
fi
