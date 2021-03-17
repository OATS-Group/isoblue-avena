#####
## Source Stage
#####
FROM --platform=$BUILDPLATFORM rust:buster as sources
WORKDIR /app

ENV USER=1000

RUN cargo init
COPY ./Cargo.toml .
COPY ./Cargo.lock .

RUN mkdir -p /app/.cargo && cargo vendor /app/vendor > /app/.cargo/config

#####
## Build Stage
#####
FROM rust:buster as builder
WORKDIR /app

ENV USER=1000

COPY ./Cargo.toml /app/Cargo.toml
COPY ./Cargo.lock /app/Cargo.lock
COPY ./src /app/src
COPY --from=sources /app/.cargo /app/.cargo
COPY --from=sources /app/vendor /app/vendor

RUN cargo build --release --offline
RUN strip /app/target/release/cand

######
### Runtime Stage
######
FROM debian:buster
WORKDIR /app

COPY --from=builder /app/target/release/cand .

ENTRYPOINT [ "./cand" ]
