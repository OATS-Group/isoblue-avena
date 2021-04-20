use chrono::NaiveDateTime;
use libc::timeval;
use std::mem;

#[derive(Debug)]
pub struct J1939MessageData {
    pub iov: Vec<u8>,
    pub control: Vec<u8>,
}

impl J1939MessageData {
    pub fn new(size: usize) -> Self {
        let iov = vec![0u8; size];

        let mut space = 0;
        unsafe {
            // Timestamp
            // TODO: We don't need to allocate timeval if J1939Socket.set_timestamp(false)
            space += libc::CMSG_SPACE(mem::size_of::<timeval>() as _);
            // Priority
            space += libc::CMSG_SPACE(mem::size_of::<u8>() as _);
            // Dest name
            space += libc::CMSG_SPACE(mem::size_of::<u64>() as _);
            // Dest addr
            space += libc::CMSG_SPACE(mem::size_of::<u8>() as _);
        }

        let control = vec![0u8; space as usize];

        J1939MessageData { iov, control }
    }
}

#[derive(Debug)]
pub struct J1939Message<'a> {
    pub timestamp: Option<NaiveDateTime>,
    pub pgn: u32,
    pub src_addr: u8,
    pub src_name: u64,
    pub dest_addr: Option<u8>,
    pub dest_name: Option<&'a [u8]>,
    pub dest_priority: Option<u8>,
    pub data: &'a [u8],
}

impl<'a> J1939Message<'a> {}
