#####
## Source Stage
#####
FROM --platform=$BUILDPLATFORM rust:slim-buster as sources
WORKDIR /app

RUN cargo init
COPY ./Cargo.toml .
COPY ./Cargo.lock .

RUN mkdir -p /app/.cargo && cargo vendor /app/vendor > /app/.cargo/config

#####
## Build Stage
#####
FROM rust:slim-buster as builder
WORKDIR /app

COPY ./Cargo.toml /app/Cargo.toml
COPY ./Cargo.lock /app/Cargo.lock
COPY ./src /app/src
COPY --from=sources /app/.cargo /app/.cargo
COPY --from=sources /app/vendor /app/vendor

RUN cargo build --release --offline

######
### Runtime Stage
######
FROM debian:buster
WORKDIR /app

RUN apt-get update && apt-get install -y cron && rm -rf /var/lib/apt/lists/*

ADD entrypoint.sh entrypoint.sh
ADD run-gps-exporter.sh run-gps-exporter.sh
RUN chmod +x run-gps-exporter.sh entrypoint.sh

COPY --from=builder /app/target/release/gps-exporter .

ENTRYPOINT ./entrypoint.sh
