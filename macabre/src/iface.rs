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

/// Read the current MAC address of an interface.
pub fn get_mac(name: &str) -> io::Result<MacAddress> {
    let fd = control_socket()?;
    let mut req = ifreq_for(name)?;
    let rc = unsafe { ioctl(fd, SIOCGIFHWADDR, &mut req) };
    unsafe { libc::close(fd) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    let raw: RawSockaddr = unsafe { std::mem::transmute(req.ifr_ifru.ifru_hwaddr) };
    let mut octets = [0u8; 6];
    octets.copy_from_slice(&raw.data[..6]);
    Ok(MacAddress::from_octets(octets))
}

/// Set the MAC address of an interface. Requires root (or CAP_NET_ADMIN).
pub fn set_mac(name: &str, mac: MacAddress) -> io::Result<()> {
    let fd = control_socket()?;
    let mut req = ifreq_for(name)?;
    let octets = mac.octets();
    let mut data = [0u8; 14];
    data[..6].copy_from_slice(&octets);
    let raw = RawSockaddr { family: 0, data };
    let sa: sockaddr = unsafe { std::mem::transmute(raw) };
    unsafe {
        req.ifr_ifru.ifru_hwaddr = sa;
    }
    let rc = unsafe { ioctl(fd, SIOCSIFHWADDR, &mut req) };
    unsafe { libc::close(fd) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Save an interface's current MAC to the store file.
pub fn save_original(name: &str, mac: MacAddress) -> io::Result<()> {
    let path = store_path()?;
    let mut table = load_table(&path)?;
    table.insert(name.to_string(), mac.to_string());
    write_table(&path, &table)
}

