FROM debian:stretch

RUN apt-get update && \
    apt-get install -y libssl1.1 gdbserver

EXPOSE 35653 2345

STOPSIGNAL SIGINT

ENV RUST_LOG=info,blockscape=debug,blockscape_core=debug,blockscape::rpc=info \
    RUST_BACKTRACE=full
    
ARG RELEASE=debug

ADD target/${RELEASE}/blockscape /

CMD [ "/blockscape", "--help" ]
