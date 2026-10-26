//! Linux network interface access via ioctl(2).

use std::ffi::{c_char, OsStr};
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;

use libc::{ifreq, ioctl, sockaddr, AF_PACKET, SIOCGIFHWADDR, SIOCSIFHWADDR, SOCK_DGRAM};

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
    if bytes.is_empty() || bytes.len() >= libc::IFNAMSIZ || bytes.contains(&0) {
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
    // Validate before opening the control socket so invalid input cannot leak an fd.
    let mut req = ifreq_for(name)?;
    let fd = control_socket()?;
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
    // Validate before opening the control socket so invalid input cannot leak an fd.
    let mut req = ifreq_for(name)?;
    let fd = control_socket()?;
    let octets = mac.octets();
    let mut data = [0u8; 14];
    data[..6].copy_from_slice(&octets);
    let raw = RawSockaddr { family: 0, data };
    let sa: sockaddr = unsafe { std::mem::transmute(raw) };
    req.ifr_ifru.ifru_hwaddr = sa;
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
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(map);
    }
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "malformed original MAC store",
        ));
    }
    let inner = trimmed[1..trimmed.len() - 1].trim();
    if inner.is_empty() {
        return Ok(map);
    }
    for pair in inner.split(',') {
        let mut kv = pair.splitn(2, ':');
        let key = kv.next().unwrap_or("").trim();
        let value = kv.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "malformed original MAC store entry")
        })?.trim();
        let key = key.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or("");
        let value = value.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or("");
        if key.is_empty() || value.parse::<MacAddress>().is_err() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid original MAC store entry",
            ));
        }
        map.insert(key.to_string(), value.to_string());
    }
    Ok(map)
}

fn write_table(
    path: &std::path::Path,
    table: &std::collections::HashMap<String, String>,
) -> io::Result<()> {
    use std::time::{SystemTime, UNIX_EPOCH};

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut body: Vec<String> = table
        .iter()
        .map(|(k, v)| format!("\"{k}\":\"{v}\""))
        .collect();
    body.sort_unstable();
    let payload = format!("{{{}}}", body.join(","));
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_path = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        nonce,
    ));
    let mut created = false;
    let result: io::Result<()> = (|| {
        let mut temp = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;
        created = true;
        temp.write_all(payload.as_bytes())?;
        temp.sync_all()?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(&tmp_path);
    }
    result
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
        assert_eq!(
            back.get("eth0").map(String::as_str),
            Some("aa:bb:cc:dd:ee:ff")
        );
    }

    #[test]
    fn write_table_creates_missing_parent_directories() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("macabre-store-parent-test-{unique}"));
        let path = root.join("nested/originals.json");
        let mut table = std::collections::HashMap::new();
        table.insert("wlan0".to_string(), "02:00:00:00:00:01".to_string());

        write_table(&path, &table).unwrap();
        let loaded = load_table(&path).unwrap();
        let _ = fs::remove_dir_all(&root);

        assert_eq!(loaded.get("wlan0").map(String::as_str), Some("02:00:00:00:00:01"));
    }

    #[test]
    fn write_table_replaces_store_without_leaving_temp_files() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("macabre-atomic-store-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("originals.json");
        let mut first = std::collections::HashMap::new();
        first.insert("wlan0".to_string(), "02:00:00:00:00:01".to_string());
        write_table(&path, &first).unwrap();

        let mut second = std::collections::HashMap::new();
        second.insert("enp2s0".to_string(), "02:00:00:00:00:02".to_string());
        write_table(&path, &second).unwrap();

        let loaded = load_table(&path).unwrap();
        let entries: Vec<_> = fs::read_dir(&root).unwrap().collect();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get("enp2s0").map(String::as_str), Some("02:00:00:00:00:02"));
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn store_serialization_is_stable_across_insertion_order() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("macabre-store-test-{unique}"));
        let first_path = base.with_extension("first.json");
        let second_path = base.with_extension("second.json");
        let mut first = std::collections::HashMap::new();
        first.insert("wlan0".to_string(), "02:00:00:00:00:01".to_string());
        first.insert("enp2s0".to_string(), "02:00:00:00:00:02".to_string());
        let mut second = std::collections::HashMap::new();
        second.insert("enp2s0".to_string(), "02:00:00:00:00:02".to_string());
        second.insert("wlan0".to_string(), "02:00:00:00:00:01".to_string());

        write_table(&first_path, &first).unwrap();
        write_table(&second_path, &second).unwrap();
        let first_text = fs::read_to_string(&first_path).unwrap();
        let second_text = fs::read_to_string(&second_path).unwrap();
        let _ = fs::remove_file(&first_path);
        let _ = fs::remove_file(&second_path);
        assert_eq!(first_text, second_text);
    }

    #[test]
    fn json_empty_and_malformed_safe() {
        assert!(serdeless_parse("{}").unwrap().is_empty());
        assert!(serdeless_parse("").unwrap().is_empty());
        assert_eq!(
            serdeless_parse("{broken").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            serdeless_parse("{\"eth0\":\"not-a-mac\"}").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn parses_multiple_original_mac_entries() {
        let table = serdeless_parse(
            "{\"wlan0\":\"02:00:00:00:00:01\",\"enp2s0\":\"02:00:00:00:00:02\"}"
        ).unwrap();
        assert_eq!(table.get("wlan0").map(String::as_str), Some("02:00:00:00:00:01"));
        assert_eq!(table.get("enp2s0").map(String::as_str), Some("02:00:00:00:00:02"));
    }

    #[test]
    fn accepts_typical_linux_interface_name() {
        assert!(ifreq_for("wlan0").is_ok());
        assert!(ifreq_for("enp2s0").is_ok());
    }

    #[test]
    fn rejects_bad_names() {
        assert!(ifreq_for("").is_err());
        assert!(ifreq_for(&"x".repeat(64)).is_err());
        assert_eq!(
            ifreq_for("wlan0\\0eth0").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn public_mac_operations_reject_bad_names_before_socket_work() {
        assert_eq!(get_mac("").unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            set_mac("", MacAddress::from_octets([2, 0, 0, 0, 0, 1]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
