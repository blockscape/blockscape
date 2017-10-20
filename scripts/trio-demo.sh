../target/debug/blockscape -h localhost -p 30001 -w $HOME/.blockscape-1 --seed-node localhost:30002 &
../target/debug/blockscape -h localhost -p 30002 -w $HOME/.blockscape-2 --seed-node localhost:30001 &
../target/debug/blockscape -h localhost -p 30003 -w $HOME/.blockscape-3 --seed-node localhost:30001
