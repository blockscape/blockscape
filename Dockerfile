FROM debian:stretch

RUN apt-get update && \
    apt-get install -y libssl1.1

ADD target/release/blockscape /

EXPOSE 35653

ENV RUST_LOG=info,blockscape=debug,blockscape_core=debug \
    RUST_BACKTRACE=1

STOPSIGNAL SIGTERM

CMD [ "/blockscape", "--help" ]
