//! Linux network interface access via ioctl(2).

use std::ffi::{c_char, OsStr};
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;

use libc::{ifreq, ioctl, sockaddr, AF_PACKET, SOCK_DGRAM, SIOCGIFHWADDR, SIOCSIFHWADDR};

use crate::mac::MacAddress;

/// Linux sockaddr layout: 2-byte family + 14-byte data (16 bytes total).
#[repr(C)]
struct RawSockaddr {
    family: u16,
    data: [u8; 14],
}

/// One network interface and its current MAC address.
#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,
    pub mac: MacAddress,
}

