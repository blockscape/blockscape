# Blockscape Engine and Reference Implementation

Welcome to Blockscape, a distributed application developer framework leveraging Blockchain technology.
The project is still pre-alpha, and lacks a comprehensive reference implementation for widespread
use. There is no public blockchain at this time. We provide a minimal checkers database application as a proof of concept.

This repository includes an in-development reference implementation--a video game--demonstrating much
of the usecase for the platform.

This project is still in active development, but we are happy to accept quality code. Please open
a merge request to do so.

## Usage

Using [Docker](https://www.docker.com/community-edition), build a simple checkers PoC DApp:

```bash
cd scripts
./build-docker
```

To run:

```bash
docker run -d --rm --net host --name blockscape dcr.buyme360.com/plu-capstone/blockscape -F -h <your network IP> --rpcbind 0.0.0.0 --seed-node <optional remote IP>:35653
```

Or for a more convienient command:

```bash
./start-blockscape-standalone.sh
```

Ensure UDP traffic on port 35653 is unfiltered between nodes.



To play checkers, first get your player ID, and optionally the opponents:

```bash
# get public key identifier for the play registered on the current node
my_id=`./run-blockscape-standalone get_my_player`
```

Play a game against yourself:

```bash
# see the current state of a checkers game
./run-blockscape-standalone get_checkers_board 0 0
# start the game and play a move
./run-blockscape-standalone new_checkers_game 0 0 $my_id
./run-blockscape-standalone play_checkers 0 0 d6 move NW
# to do a 3-point jump
./run-blockscape-standalone play_checkers 0 0 d6 jump NW SW NW
```

Invalid moves will be denied.


