use derive_more::{Display, From};
use serde::Serialize;
use socketcan::CanFrame;
use std::time::SystemTime;

#[derive(Serialize, Display, From, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Priority(u8);

#[derive(Serialize, Display, From, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PGN(u32);

#[derive(Serialize, Display, From, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Source(u8);

#[derive(Serialize, Display, From, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Destination(u8);

#[derive(From, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Data(Vec<u8>);

impl Serialize for Data {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(&self.0))
    }
}

impl std::fmt::Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

#[derive(Serialize, Debug)]
pub struct Message {
    pub ts: f64,
    pub pgn: PGN,
    pub source: Source,
    pub dest: Option<Destination>,
    pub priority: Priority,
    pub data: Data,
}

impl From<(SystemTime, CanFrame)> for Message {
    fn from((time, frame): (SystemTime, CanFrame)) -> Self {
        // TODO: I think this j1939 parser is right?
        let id = frame.id();

        let sa = id as u8;
        let pdu = (id >> 8) & 0x3FFFF;
        let pf = (pdu >> 8) as u8;

        let pgn;
        let da;
        if pf >= 240 {
            pgn = pdu;
            da = None;
        } else {
            pgn = (pf as u32) << 8;
            da = Some(pdu as u8);
        }

        let ts = time
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("System time is invalid.")
            .as_secs_f64();

        Message {
            ts,
            priority: Priority(((id >> 26) & 0x07) as u8),
            pgn: PGN(pgn),
            source: Source(sa),
            dest: da.map(|da| Destination(da)),
            data: Data(frame.data().to_owned()),
        }
    }
}
