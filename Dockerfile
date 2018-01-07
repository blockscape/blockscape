FROM debian:stretch

RUN apt-get update && \
    apt-get install -y libssl1.1

ADD target/release/blockscape /

EXPOSE 35653

ENV RUST_LOG=info,blockscape=debug,blockscape_core=debug,blockscape::rpc=info \
    RUST_BACKTRACE=1

STOPSIGNAL SIGINT

CMD [ "/blockscape", "--help" ]
