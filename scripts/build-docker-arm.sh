set -e

DIR=$(readlink -e $(dirname $0)/..)

if docker ps -a | grep blockscape_build_arm; then
    docker start -i blockscape_build_arm
else
    docker run -i --name blockscape_build_arm --sig-proxy=true -v $DIR:/src dcr.buyme360.com/plu-capstone/rust/arm:latest cargo build --manifest-path=/src/checkers/Cargo.toml -j8 --verbose
fi

docker build -t dcr.buyme360.com/plu-capstone/blockscape:arm ..
