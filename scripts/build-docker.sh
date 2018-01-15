set -e

DIR=$(readlink -e $(dirname $0)/..)

if docker ps -a | grep blockscape_build; then
    docker start -i blockscape_build
else
    docker run -i --name blockscape_build --sig-proxy=true -v $HOME/.cargo:/root/.cargo -v $DIR:/src rust:1.21-stretch cargo build --manifest-path=/src/Cargo.toml -j8
fi

docker build -t dcr.buyme360.com/plu-capstone/blockscape:latest ..
