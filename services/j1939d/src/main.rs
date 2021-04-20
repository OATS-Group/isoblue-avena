mod j1939;

use anyhow::Context;
use canparse::pgn::{ParseMessage, PgnLibrary};
use j1939::J1939Socket;
use serde_json::json;

use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};

use prometheus::{
    core::{AtomicF64, GenericGauge},
    opts, register_gauge, Encoder, TextEncoder,
};

use std::{str::FromStr, thread};
use tiny_http::{Header, HeaderField, Response, Server};

use anyhow::Result;

fn main() -> Result<()> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("can_interface")
                .index(1)
                .env("CAN_INTERFACE")
                .takes_value(true)
                .help("SocketCAN interface to monitor"),
        )
        .arg(
            Arg::with_name("dbc_file")
                .index(2)
                .env("DBC_FILE")
                .takes_value(true)
                .help("DBC file to decode with"),
        )
        .arg(
            Arg::with_name("nats_server")
                .index(3)
                .env("NATS_SERVER")
                .takes_value(true)
                .help("NATS connection string"),
        )
        .arg(
            Arg::with_name("nats_sub_prefix")
                .long("sub_prefix")
                .env("NATS_SUB_PREFIX")
                .takes_value(true)
                .default_value("j1939")
                .help("NATS subject prefix"),
        )
        .arg(
            Arg::with_name("prom_listen")
                .long("prom_listen")
                .env("PROM_LISTEN")
                .takes_value(true)
                .default_value("0.0.0.0:10000")
                .help("Listen string for Prometheus reporting"),
        )
        .get_matches();

    let can_interface = matches
        .value_of("can_interface")
        .with_context(|| "SocketCAN interface not provided")?;

    let nats_server = matches
        .value_of("nats_server")
        .with_context(|| "NATS connection string not provided")?;

    let nats_sub_prefix = matches
        .value_of("nats_sub_prefix")
        .with_context(|| "NATS subject prefix not provided?")?;

    let prom_listen = matches
        .value_of("prom_listen")
        .with_context(|| "Prom listen not provided?")?
        .to_owned();

    // TODO: make not required?
    let dbc_file = matches
        .value_of("dbc_file")
        .with_context(|| "No dbc file was provided?")?;

    println!("Connecting to NATS");
    let nc = nats::connect(nats_server)
        .with_context(|| format!("Could not connect to NATS at {}", nats_server))?;
    println!("Connected to NATS");

    println!("Connecting to CAN");
    let mut s = J1939Socket::open(can_interface)
        .with_context(|| format!("Could not connect to can interface {}", can_interface))?;
    s.set_promisc(true)?;
    s.set_broadcast(true)?;
    s.set_timestamp(true)?;
    println!("Connected to CAN");

    println!("Loading DBC file");
    let lib = PgnLibrary::from_dbc_file(dbc_file)?;
    println!("Loaded DBC file");

    let mut gauges = std::collections::HashMap::<String, GenericGauge<AtomicF64>>::new();

    thread::spawn(move || -> Result<()> {
        loop {
            let server = Server::http(prom_listen.clone()).unwrap();

            for request in server.incoming_requests() {
                println!("Prometheus polled us.");

                let encoder = TextEncoder::new();
                let metrics = prometheus::gather();
                let mut buffer = vec![];

                encoder.encode(&metrics, &mut buffer)?;

                let mut response = Response::from_data(buffer);
                response.add_header(Header {
                    field: HeaderField::from_str("Content-Type").unwrap(),
                    value: ascii::AsciiString::from_str(encoder.format_type())?,
                });

                request.respond(response)?;
            }
        }
    });

    loop {
        let msg = s.recv()?;

        if let Some(pgn_def) = lib.get_pgn(msg.pgn) {
            for (s, spn) in pgn_def.get_spns().iter() {
                if let Some(value) = spn.parse_message(msg.data) {
                    let sub = format!("{}.{}", nats_sub_prefix, s);
                    nc.publish(
                    &sub,
                    serde_json::to_vec(&json!({
                        "pgn": msg.pgn,
                        "name": s,
                        "time": msg.timestamp.map(|t| t.timestamp_nanos() as f64 / 1_000_000_000.0),
                        "value": value,
                        "units": spn.units,
                        "min_value": spn.min_value,
                        "max_value": spn.max_value
                    }))
                    .with_context(|| {
                        "Failed to
                seralize frame"
                    })?,
                )
                .with_context(|| "Failed to send parsed j1939 frame to NATS")?;

                    // TODO: Figure out these clones
                    let gauge = gauges.entry(spn.name.clone()).or_insert_with(|| {
                        let opts = opts!(
                            format!("{}_{}", nats_sub_prefix, s),
                            spn.description.clone()
                        );
                        register_gauge!(opts).unwrap()
                    });

                    gauge.set(value as f64);
                }
            }
        }
    }
}
