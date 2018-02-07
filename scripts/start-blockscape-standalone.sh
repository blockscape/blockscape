#!/bin/bash

GATEWAY=$(docker inspect scripts_blockscape_1_1 | jq -r '.[0].NetworkSettings.Networks.scripts_net.Gateway')
SEED=$(docker inspect scripts_blockscape_1_1 | jq -r '.[0].NetworkSettings.Networks.scripts_net.IPAddress')

if [ $# -gt 0 ]; then
	docker run -d --rm --name blockscape --net host dcr.buyme360.com/plu-capstone/blockscape:latest /blockscape --rpcbind 0.0.0.0 "$@"
elif [  $GATEWAY != 'null' ]; then
	docker run -d --rm --name blockscape --net host dcr.buyme360.com/plu-capstone/blockscape:latest /blockscape --host $GATEWAY --seed-node $SEED:35653 --rpcbind 0.0.0.0
else
	docker run -d --rm --name blockscape --net host dcr.buyme360.com/plu-capstone/blockscape:latest /blockscape --host 127.0.0.1 --rpcbind 0.0.0.0
fi
