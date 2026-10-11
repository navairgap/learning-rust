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

/// All non-loopback interfaces found in /sys/class/net, with their MACs.
pub fn list_interfaces() -> io::Result<Vec<Interface>> {
    let mut out = Vec::new();
    for entry in fs::read_dir("/sys/class/net")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        if let Ok(mac) = get_mac(&name) {
            out.push(Interface { name, mac });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Open a control socket for interface ioctls.
fn control_socket() -> io::Result<i32> {
    let fd = unsafe { libc::socket(AF_PACKET, SOCK_DGRAM, 0) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

/// Build a zeroed ifreq with the interface name filled in.
fn ifreq_for(name: &str) -> io::Result<ifreq> {
    let bytes = OsStr::new(name).as_bytes();
    if bytes.is_empty() || bytes.len() >= libc::IFNAMSIZ {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bad interface name {name:?}"),
        ));
    }
    let mut req: ifreq = unsafe { std::mem::zeroed() };
    for (i, &b) in bytes.iter().enumerate() {
        req.ifr_name[i] = b as c_char;
    }
    Ok(req)
}

