mod control_messages;
mod message;
mod socket;

pub use self::control_messages::*;
pub use self::message::*;
pub use self::socket::*;

use libc::{c_int, CAN_J1939, SOL_CAN_BASE};

// from j1939.h
pub const SO_J1939_PROMISC: c_int = 2;
pub const SOL_CAN_J1939: c_int = SOL_CAN_BASE + CAN_J1939;

// from j1939.h
const SCM_J1939_DEST_ADDR: c_int = 1;
const SCM_J1939_DEST_NAME: c_int = 2;
const SCM_J1939_PRIO: c_int = 3;
//pub const SCM_J1939_ERRQUEUE: c_int = 4;
