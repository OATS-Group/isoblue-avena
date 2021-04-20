use super::{
    ControlMessage, ControlMessageIterator, J1939Message, J1939MessageData, SOL_CAN_J1939,
    SO_J1939_PROMISC,
};
use libc::{
    __c_anonymous_sockaddr_can_can_addr, __c_anonymous_sockaddr_can_j1939, c_int, c_uint, c_ushort,
    close, if_nametoindex, iovec, msghdr, recvmsg, setsockopt, sockaddr, sockaddr_can, socket,
    AF_CAN, CAN_J1939, PATH_MAX, PF_CAN, SOCK_DGRAM, SOL_SOCKET, SO_BROADCAST, SO_TIMESTAMP,
};
use std::ffi;
use std::io;
use std::ptr;
use thiserror::Error;

type Result<T> = std::result::Result<T, J1939SocketError>;

#[derive(Error, Debug)]
pub enum J1939SocketError {
    #[error("Invalid interface name")]
    PathError,

    #[error("Could not open SocketCAN interface")]
    OpenError { source: io::Error },

    #[error("Count not receive from SocketCAN")]
    RecvError { source: io::Error },

    #[error(transparent)]
    IOError {
        #[from]
        source: io::Error,
    },
}

pub struct J1939Socket {
    fd: c_int,
    addr: sockaddr_can,
    data: J1939MessageData,
}

impl J1939Socket {
    pub fn open(ifname: &str) -> Result<J1939Socket> {
        // Make sure we can hold the ifname string
        if ifname.len() >= PATH_MAX as usize {
            return Err(J1939SocketError::PathError);
        }
        // A zero'ed buffer to hold the C version of the `ifname` string
        let mut buf = [0u8; PATH_MAX as usize];

        let ifname = ifname.as_bytes();

        // Make sure the string doesn't have a null in it
        match ifname.iter().position(|c| *c == 0) {
            Some(_) => return Err(J1939SocketError::PathError),
            None => {
                let if_index = unsafe {
                    // Copy into zero'd buffer so string ends in null (for c)
                    ptr::copy_nonoverlapping(ifname.as_ptr(), buf.as_mut_ptr(), ifname.len());
                    let c_ifname = ffi::CStr::from_bytes_with_nul_unchecked(&buf);

                    // Get interface index
                    if_nametoindex(c_ifname.as_ptr())
                };

                Ok(Self::open_if(if_index)?)
            }
        }
    }

    pub fn open_if(if_index: c_uint) -> Result<J1939Socket> {
        let fd = unsafe { socket(PF_CAN, SOCK_DGRAM, CAN_J1939) };
        if fd < 0 {
            return Err(J1939SocketError::OpenError {
                source: io::Error::last_os_error(),
            });
        }

        let recvbuf_rv;
        unsafe {
            let value = 8;
            let value_ptr = &value as *const libc::c_int;
            recvbuf_rv = libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                value_ptr as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as u32,
            )
        }

        if recvbuf_rv < 0 {}

        // Use j1939 SocketCAN with no filtering
        let addr = sockaddr_can {
            can_family: AF_CAN as c_ushort,
            can_ifindex: if_index as i32,
            can_addr: __c_anonymous_sockaddr_can_can_addr {
                j1939: __c_anonymous_sockaddr_can_j1939 {
                    name: 0,
                    pgn: 0x40000,
                    addr: 0xff,
                },
            },
        };

        // Bind to j1939
        let res = unsafe {
            libc::bind(
                fd,
                (&addr as *const sockaddr_can) as *const sockaddr,
                std::mem::size_of::<sockaddr_can>() as u32,
            )
        };

        if res < 0 {
            // close manually because J1939Socket is not yet created for `Drop`
            // to clean up
            unsafe { close(fd) };

            return Err(J1939SocketError::OpenError {
                source: io::Error::last_os_error(),
            });
        }

        // TODO: Export size to open
        let data = J1939MessageData::new(1024);

        Ok(J1939Socket { fd, addr, data })
    }

    // Borrow mut so J1939Socket can't be used anymore
    pub fn close(&mut self) -> Result<()> {
        let res = unsafe { close(self.fd) };

        if res != -1 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(())
    }

    pub fn set_promisc(&self, promisc: bool) -> Result<()> {
        let res = unsafe {
            setsockopt(
                self.fd,
                SOL_CAN_J1939,
                SO_J1939_PROMISC,
                &(promisc as i32) as *const c_int as *const _,
                std::mem::size_of::<libc::c_int>() as u32,
            )
        };

        if res < 0 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(())
    }

    pub fn set_broadcast(&self, broadcast: bool) -> Result<()> {
        let res = unsafe {
            setsockopt(
                self.fd,
                SOL_SOCKET,
                SO_BROADCAST,
                &(broadcast as i32) as *const c_int as *const _,
                std::mem::size_of::<libc::c_int>() as u32,
            )
        };

        if res < 0 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(())
    }

    pub fn set_timestamp(&self, timestamp: bool) -> Result<()> {
        let res = unsafe {
            setsockopt(
                self.fd,
                SOL_SOCKET,
                SO_TIMESTAMP,
                &(timestamp as i32) as *const c_int as *const _,
                std::mem::size_of::<libc::c_int>() as u32,
            )
        };

        if res < 0 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(())
    }

    // NOTE need recv to take a lifetime so we can lifetime the output
    pub fn recv(&mut self) -> Result<J1939Message> {
        let mut iovec = iovec {
            iov_base: self.data.iov.as_mut_ptr() as *mut _,
            iov_len: self.data.iov.len(),
        };

        let mut msg = msghdr {
            msg_name: (&self.addr as *const libc::sockaddr_can) as *mut _,
            msg_namelen: std::mem::size_of::<libc::sockaddr_can>() as u32,
            msg_iov: &mut iovec as *mut _,
            msg_iovlen: 1,
            msg_control: self.data.control.as_mut_ptr() as *mut _,
            msg_controllen: self.data.control.len(),
            msg_flags: 0,
        };
        let res = unsafe { recvmsg(self.fd, &mut msg as *mut msghdr, 0) };

        if res < 0 {
            return Err(J1939SocketError::RecvError {
                source: io::Error::last_os_error(),
            });
        }

        let cms = ControlMessageIterator::new(&msg);

        let mut dest_addr = None;
        let mut dest_name = None;
        let mut dest_priority = None;
        let mut timestamp = None;

        for cm in cms {
            match cm {
                ControlMessage::DestAddr(addr) => dest_addr = Some(addr),
                ControlMessage::DestName(name) => dest_name = Some(name),
                ControlMessage::Priority(priority) => dest_priority = Some(priority),
                ControlMessage::Timestamp(time) => timestamp = Some(time),
                ControlMessage::Unknown() => {}
            }
        }

        Ok(J1939Message {
            pgn: unsafe { self.addr.can_addr.j1939.pgn },
            src_addr: unsafe { self.addr.can_addr.j1939.addr },
            src_name: unsafe { self.addr.can_addr.j1939.name },
            data: &self.data.iov[0..res as usize],
            dest_addr,
            dest_name,
            dest_priority,
            timestamp,
        })
    }
}

impl<'a> Drop for J1939Socket {
    fn drop(&mut self) {
        // ignore result, we tried out best
        self.close().ok();
    }
}
