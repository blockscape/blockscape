FROM debian:stretch

RUN apt-get update && \
    apt-get install -y libssl1.1 gdbserver

ADD target/debug/blockscape /

EXPOSE 35653
EXPOSE 2345

ENV RUST_LOG=info,blockscape=debug,blockscape_core=debug,blockscape::rpc=info \
    RUST_BACKTRACE=1

STOPSIGNAL SIGINT

CMD [ "/blockscape", "--help" ]
