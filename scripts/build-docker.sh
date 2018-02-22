set -e

DIR=$(readlink -e $(dirname $0)/..)

if docker ps -a | grep blockscape_build_x86; then
    docker start -i blockscape_build_x86
else
    docker run -i --name blockscape_build_x86 --sig-proxy=true -e LIBCLANG_PATH=/usr/lib/llvm-3.8/lib -v $HOME/.cargo:/root/.cargo -v $DIR:/src dcr.buyme360.com/plu-capstone/rust:latest cargo build --manifest-path=/src/checkers/Cargo.toml -j8
fi

docker build -t dcr.buyme360.com/plu-capstone/blockscape:latest --build-arg PRODUCT=checkers ..
