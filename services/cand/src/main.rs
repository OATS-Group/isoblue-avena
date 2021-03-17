mod j1939;

use anyhow::{Context, Result};
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use j1939::Message;
use nats;
use socketcan::CanSocket;

// TODO: Deal with NEMA 2000
// TODO: Deal with standard can frames
// TODO: Deal with TP / ETP of J1939 (can we just use the j1393 kernel module?)
//         -- I suppose this would require upgrading socketcan to support
//            opening socketcan with that flag.

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
            Arg::with_name("nats_server")
                .index(2)
                .env("NATS_SERVER")
                .takes_value(true)
                .help("NATS connection string"),
        )
        .get_matches();

    let can_interface = matches
        .value_of("can_interface")
        .with_context(|| "SocketCAN interface not provided")?;

    let nats_server = matches
        .value_of("nats_server")
        .with_context(|| "NATS connection string not provided")?;

    println!("Connecting to NATS");
    let nc = nats::connect(nats_server)
        .with_context(|| format!("Could not connect to NATS at {}", nats_server))?;
    println!("Connected to NATS");

    println!("Connecting to CAN");
    let mut s = CanSocket::open(can_interface)
        .with_context(|| format!("Could not connect to can interface {}", can_interface))?;
    println!("Connected to CAN");

    // TODO: Ensure CAN interface is configured and up? Auto baud?

    let mut i = 0;
    loop {
        let (f, ts) = s
            .read_frame_with_timestamp()
            .with_context(|| "Failed to read from socketcan.")?;

        i = i + 1;
        if i > 1000 {
            println!("Processed 1000 can messages");
            i = 0;
        }
        // Is j1939?
        if f.is_extended() && !f.is_rtr() {
            // TODO: Make a PR to socktcan to move ts into CanFrame
            let msg = Message::from((ts, f));

            let sub = format!(
                "j1939.{}.{}.{}",
                msg.pgn,
                msg.source,
                match msg.dest {
                    Some(da) => da.to_string(),
                    None => "NONE".to_owned(),
                }
            );

            nc.publish(
                &sub,
                serde_json::to_vec(&msg).with_context(|| "Failed to serialize J1939 frame.")?,
            )
            .with_context(|| "Failed send J1939 frame to NATS")?;
        }
    }
}
