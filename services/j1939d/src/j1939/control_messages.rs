use super::{SCM_J1939_DEST_ADDR, SCM_J1939_DEST_NAME, SCM_J1939_PRIO, SOL_CAN_J1939};
use chrono::NaiveDateTime;
use libc::{cmsghdr, msghdr, SCM_TIMESTAMP, SOL_SOCKET};
use std::ptr;
use std::slice;

#[derive(Debug)]
pub enum ControlMessage<'a> {
    DestAddr(u8),
    DestName(&'a [u8]),
    Priority(u8),
    Timestamp(NaiveDateTime),
    Unknown(),
}

/// Convert the low level cmsg data structures into a J1939 focused enum
impl<'a> From<*const cmsghdr> for ControlMessage<'a> {
    fn from(hdr: *const cmsghdr) -> Self {
        let h = unsafe { *hdr };
        match (h.cmsg_level, h.cmsg_type) {
            (SOL_CAN_J1939, SCM_J1939_DEST_ADDR) => {
                let p = unsafe { libc::CMSG_DATA(hdr) };
                let addr = unsafe { ptr::read_unaligned(p as *const _) };

                Self::DestAddr(addr)
            }
            (SOL_CAN_J1939, SCM_J1939_PRIO) => {
                let p = unsafe { libc::CMSG_DATA(hdr) };
                let priority = unsafe { ptr::read_unaligned(p as *const _) };
                Self::Priority(priority)
            }
            (SOL_CAN_J1939, SCM_J1939_DEST_NAME) => {
                let p = unsafe { libc::CMSG_DATA(hdr) };
                let len = hdr as *const _ as usize + h.cmsg_len as usize - p as usize;
                let name = unsafe { slice::from_raw_parts(p, len) };

                Self::DestName(name)
            }
            (SOL_SOCKET, SCM_TIMESTAMP) => {
                let p = unsafe { libc::CMSG_DATA(hdr) };
                let tv: libc::timeval = unsafe { ptr::read_unaligned(p as *const _) };
                let time = NaiveDateTime::from_timestamp(tv.tv_sec.into(), tv.tv_usec as u32 * 1000);

                Self::Timestamp(time)
            }
            (_, _) => Self::Unknown(),
        }
    }
}

/// A convinence iterator for decoding CMSG data into ControlMessage
pub struct ControlMessageIterator<'a> {
    msghdr: &'a msghdr,
    cmsghdr: Option<&'a cmsghdr>,
}

impl<'a> ControlMessageIterator<'a> {
    pub fn new(msghdr: *const msghdr) -> Self {
        ControlMessageIterator {
            msghdr: unsafe { &*msghdr },
            cmsghdr: Some(unsafe { &*libc::CMSG_FIRSTHDR(msghdr) }),
        }
    }
}

impl<'a> Iterator for ControlMessageIterator<'a> {
    type Item = ControlMessage<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.cmsghdr {
            None => None,
            Some(hdr) => {
                let cm = Some(ControlMessage::from(hdr as *const _));
                self.cmsghdr =
                    unsafe { libc::CMSG_NXTHDR(self.msghdr as *const _, hdr as *const _).as_ref() };
                cm
            }
        }
    }
}
