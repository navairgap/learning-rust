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

/// Read the saved original MAC for an interface, if any.
pub fn load_original(name: &str) -> io::Result<Option<MacAddress>> {
    let path = store_path()?;
    let table = load_table(&path)?;
    match table.get(name) {
        Some(s) => match s.parse() {
            Ok(mac) => Ok(Some(mac)),
            Err(_) => Ok(None),
        },
        None => Ok(None),
    }
}

/// Remove the saved original MAC for an interface (after a successful reset).
pub fn clear_original(name: &str) -> io::Result<()> {
    let path = store_path()?;
    let mut table = load_table(&path)?;
    table.remove(name);
    write_table(&path, &table)
}

fn store_path() -> io::Result<std::path::PathBuf> {
    let dir = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no HOME or XDG_DATA_HOME"))?;
    Ok(dir.join("macabre/originals.json"))
}

fn load_table(path: &std::path::Path) -> io::Result<std::collections::HashMap<String, String>> {
    match fs::read_to_string(path) {
        Ok(text) => serdeless_parse(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(std::collections::HashMap::new()),
        Err(e) => Err(e),
    }
}

/// Minimal JSON object reader: {"name":"aa:bb:..."} — enough for this store.
fn serdeless_parse(text: &str) -> io::Result<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    let trimmed = text
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    if trimmed.is_empty() {
        return Ok(map);
    }
    for pair in trimmed.split(',') {
        let mut kv = pair.splitn(2, ':');
        let k = kv.next().unwrap_or("").trim().trim_matches('"');
        let v = kv.next().unwrap_or("").trim().trim_matches('"');
        if !k.is_empty() {
            map.insert(k.to_string(), v.to_string());
        }
    }
    Ok(map)
}

fn write_table(
    path: &std::path::Path,
    table: &std::collections::HashMap<String, String>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body: Vec<String> = table
        .iter()
        .map(|(k, v)| format!("\"{k}\":\"{v}\""))
        .collect();
    fs::write(path, format!("{{{}}}", body.join(",")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip() {
        let mut m = std::collections::HashMap::new();
        m.insert("eth0".to_string(), "aa:bb:cc:dd:ee:ff".to_string());
        let text = format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("\"{k}\":\"{v}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        let back = serdeless_parse(&text).unwrap();
        assert_eq!(back.get("eth0").map(String::as_str), Some("aa:bb:cc:dd:ee:ff"));
    }

    #[test]
    fn json_empty_and_malformed_safe() {
        assert!(serdeless_parse("{}").unwrap().is_empty());
        assert!(serdeless_parse("").unwrap().is_empty());
        assert!(serdeless_parse("{broken").is_ok());
    }

    #[test]
    fn rejects_bad_names() {
        assert!(ifreq_for("").is_err());
        assert!(ifreq_for(&"x".repeat(64)).is_err());
    }
}
