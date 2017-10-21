FROM debian:stretch

RUN apt-get update && \
    apt-get install -y libssl1.1

ADD target/debug/blockscape /

EXPOSE 35653

ENV RUST_LOG=debug \
    RUST_BACKTRACE=1

STOPSIGNAL SIGTERM

CMD [ "/blockscape", "--help" ]