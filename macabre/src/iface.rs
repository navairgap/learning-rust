//! Linux network interface access via ioctl(2).

use std::ffi::{c_char, OsStr};
use std::fs;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;

use libc::{ifreq, ioctl, sockaddr, SIOCGIFHWADDR, SIOCSIFHWADDR, SOCK_DGRAM};

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

/// Interface name from a directory entry, if it is valid UTF-8.
///
/// An interface name is never shown to the kernel mangled: entries whose
/// names are not valid UTF-8 are skipped rather than lossy-replaced, which
/// would query an interface that does not exist.
fn entry_name(file_name: &std::ffi::OsStr) -> Option<String> {
    file_name.to_str().map(str::to_string)
}

/// All non-loopback interfaces found in /sys/class/net, with their MACs.
pub fn list_interfaces() -> io::Result<Vec<Interface>> {
    let mut out = Vec::new();
    for entry in fs::read_dir("/sys/class/net")? {
        let entry = entry?;
        let Some(name) = entry_name(&entry.file_name()) else {
            continue;
        };
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
///
/// `SOCK_DGRAM/AF_INET` is enough for `SIOCGIFHWADDR`/`SIOCSIFHWADDR`: those
/// ioctls are dispatched by the network layer regardless of socket family.
/// `AF_PACKET` would additionally require CAP_NET_RAW just to create the
/// socket, locking unprivileged `list`/`get` out entirely.
fn control_socket() -> io::Result<OwnedFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, SOCK_DGRAM, 0) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // OwnedFd closes the descriptor on every return path, including ioctl errors.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
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
    let rc = unsafe { ioctl(fd.as_raw_fd(), SIOCGIFHWADDR, &mut req) };
    let result = if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    };
    result?;
    let raw: RawSockaddr = unsafe { std::mem::transmute(req.ifr_ifru.ifru_hwaddr) };
    let mut octets = [0u8; 6];
    octets.copy_from_slice(&raw.data[..6]);
    Ok(MacAddress::from_octets(octets))
}

/// Build the sockaddr carrying the new MAC for SIOCSIFHWADDR.
///
/// The family must match the device type (ARPHRD_ETHER for typical
/// interfaces): modern kernels reject the ioctl with EINVAL when
/// `sa_family != dev->type`, and AF_UNSPEC is no longer accepted.
fn hwaddr_sockaddr(mac: MacAddress) -> sockaddr {
    let octets = mac.octets();
    let mut data = [0u8; 14];
    data[..6].copy_from_slice(&octets);
    let raw = RawSockaddr {
        family: libc::ARPHRD_ETHER as u16,
        data,
    };
    unsafe { std::mem::transmute(raw) }
}

/// Set the MAC address of an interface. Requires root (or CAP_NET_ADMIN).
pub fn set_mac(name: &str, mac: MacAddress) -> io::Result<()> {
    // Validate before opening the control socket so invalid input cannot leak an fd.
    let mut req = ifreq_for(name)?;
    let fd = control_socket()?;
    req.ifr_ifru.ifru_hwaddr = hwaddr_sockaddr(mac);
    let rc = unsafe { ioctl(fd.as_raw_fd(), SIOCSIFHWADDR, &mut req) };
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

/// Resolve the base directory for the saved-MAC store.
///
/// Per the XDG Base Directory spec, a relative XDG_DATA_HOME is invalid and
/// must be ignored in favor of the HOME default.
fn data_home(
    xdg: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> io::Result<std::path::PathBuf> {
    let base = match xdg.map(std::path::PathBuf::from) {
        Some(dir) if dir.is_absolute() => dir,
        _ => home
            .map(|h| std::path::PathBuf::from(h).join(".local/share"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no HOME or XDG_DATA_HOME"))?,
    };
    Ok(base.join("macabre"))
}

fn store_path() -> io::Result<std::path::PathBuf> {
    data_home(
        std::env::var_os("XDG_DATA_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
    .map(|dir| dir.join("originals.json"))
}

fn load_table(path: &std::path::Path) -> io::Result<std::collections::HashMap<String, String>> {
    match fs::read_to_string(path) {
        Ok(text) => serdeless_parse(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(std::collections::HashMap::new()),
        Err(e) => Err(e),
    }
}

/// Escape a store key or value.
///
/// Linux interface names may contain any byte except '/' and NUL, so the
/// separators this hand-rolled JSON relies on (`"`, `,`, `:`, `\`) can all
/// appear inside a name and must be escaped to keep the store round-trippable.
fn escape_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '"' | ',' | ':' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Read one escaped-quoted string from the front of `s`.
///
/// Returns the unescaped content and the remainder after the closing quote.
/// A `split(',')`-style parse cannot work here: escaped separators may
/// appear inside keys and would split a single entry in two.
fn take_quoted(s: &str) -> Option<(String, &str)> {
    let rest = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &rest[i + 1..])),
            '\\' => match chars.next() {
                Some((_, e @ ('\\' | '"' | ',' | ':'))) => out.push(e),
                _ => return None,
            },
            c => out.push(c),
        }
    }
    None
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
    let malformed = || io::Error::new(io::ErrorKind::InvalidData, "malformed original MAC store");
    let mut rest = inner;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let (key, remainder) = take_quoted(rest).ok_or_else(malformed)?;
        rest = remainder.trim_start();
        rest = rest.strip_prefix(':').ok_or_else(malformed)?;
        rest = rest.trim_start();
        let (value, remainder) = take_quoted(rest).ok_or_else(malformed)?;
        rest = remainder.trim_start();
        match rest.strip_prefix(',') {
            Some(remainder) => rest = remainder,
            None if rest.is_empty() => {}
            None => return Err(malformed()),
        }
        if key.is_empty() || value.parse::<MacAddress>().is_err() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid original MAC store entry",
            ));
        }
        if map.contains_key(&key) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate interface in original MAC store",
            ));
        }
        map.insert(key, value);
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
        .map(|(k, v)| format!("\"{}\":\"{}\"", escape_key(k), escape_key(v)))
        .collect();
    body.sort_unstable();
    let payload = format!("{{{}}}", body.join(","));
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_path =
        path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce,));
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
    fn missing_original_store_is_treated_as_empty() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("macabre-missing-store-{unique}.json"));
        assert!(!path.exists());
        assert!(load_table(&path).unwrap().is_empty());
    }

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

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macabre-store-parent-test-{unique}"));
        let path = root.join("nested/originals.json");
        let mut table = std::collections::HashMap::new();
        table.insert("wlan0".to_string(), "02:00:00:00:00:01".to_string());

        write_table(&path, &table).unwrap();
        let loaded = load_table(&path).unwrap();
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            loaded.get("wlan0").map(String::as_str),
            Some("02:00:00:00:00:01")
        );
    }

    #[test]
    fn write_table_replaces_store_without_leaving_temp_files() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
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
        assert_eq!(
            loaded.get("enp2s0").map(String::as_str),
            Some("02:00:00:00:00:02")
        );
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn store_serialization_is_stable_across_insertion_order() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
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
            serdeless_parse("{\"eth0\":\"not-a-mac\"}")
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn duplicate_interface_entries_are_rejected() {
        let text = "{\"eth0\":\"02:00:00:00:00:01\",\"eth0\":\"02:00:00:00:00:02\"}";
        assert_eq!(
            serdeless_parse(text).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn parses_multiple_original_mac_entries() {
        let table =
            serdeless_parse("{\"wlan0\":\"02:00:00:00:00:01\",\"enp2s0\":\"02:00:00:00:00:02\"}")
                .unwrap();
        assert_eq!(
            table.get("wlan0").map(String::as_str),
            Some("02:00:00:00:00:01")
        );
        assert_eq!(
            table.get("enp2s0").map(String::as_str),
            Some("02:00:00:00:00:02")
        );
    }

    #[test]
    fn accepts_typical_linux_interface_name() {
        assert!(ifreq_for("wlan0").is_ok());
        assert!(ifreq_for("enp2s0").is_ok());
    }

    #[test]
    fn interface_name_length_uses_the_exact_ifreq_boundary() {
        let maximum = "x".repeat(libc::IFNAMSIZ - 1);
        let too_long = "x".repeat(libc::IFNAMSIZ);
        assert!(ifreq_for(&maximum).is_ok());
        assert_eq!(
            ifreq_for(&too_long).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn rejects_bad_names() {
        assert!(ifreq_for("").is_err());
        assert!(ifreq_for(&"x".repeat(64)).is_err());
        assert_eq!(
            ifreq_for("wlan0\0eth0").unwrap_err().kind(),
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
    #[test]
    fn keys_with_json_separators_roundtrip_through_the_store() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macabre-escape-store-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("originals.json");
        // Every character the hand-rolled JSON treats as a separator.
        let hostile = "a:b, c\"d\\e";
        let mut table = std::collections::HashMap::new();
        table.insert(hostile.to_string(), "02:00:00:00:00:01".to_string());

        write_table(&path, &table).unwrap();
        let loaded = load_table(&path).unwrap();
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            loaded.get(hostile).map(String::as_str),
            Some("02:00:00:00:00:01")
        );
    }

    #[test]
    fn dangling_escapes_are_rejected_not_silently_dropped() {
        assert_eq!(take_quoted("\"trailing\\\""), None);
        assert_eq!(take_quoted("\"bad\\xescape\""), None);
        let (out, rest) = take_quoted("\"a\\:b\\\"c\"tail").unwrap();
        assert_eq!(out, "a:b\"c");
        assert_eq!(rest, "tail");
    }

    #[test]
    fn data_home_prefers_absolute_xdg_overrides() {
        use std::ffi::OsStr;

        let home = OsStr::new("/home/someone");
        assert_eq!(
            data_home(Some(OsStr::new("/xdg/data")), Some(home)).unwrap(),
            std::path::PathBuf::from("/xdg/data/macabre")
        );
    }

    #[test]
    fn data_home_ignores_relative_xdg_overrides() {
        use std::ffi::OsStr;

        let home = OsStr::new("/home/someone");
        // Per the XDG spec a relative XDG_DATA_HOME is invalid, not
        // cwd-relative: fall back to HOME.
        assert_eq!(
            data_home(Some(OsStr::new("relative/path")), Some(home)).unwrap(),
            std::path::PathBuf::from("/home/someone/.local/share/macabre")
        );
    }

    #[test]
    fn data_home_without_any_base_is_an_error() {
        assert_eq!(
            data_home(None, None).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn non_utf8_interface_names_are_skipped_not_mangled() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        assert_eq!(
            entry_name(OsStr::new("wlan0")),
            Some("wlan0".to_string())
        );
        // 0x80 alone is invalid UTF-8; a lossy conversion would query a
        // replacement-character name that the kernel does not know.
        let invalid = OsStr::from_bytes(b"\x80bad");
        assert_eq!(entry_name(invalid), None);
    }

    #[test]
    fn hwaddr_sockaddr_carries_ether_family_and_octets() {
        let sa = hwaddr_sockaddr(MacAddress::from_octets([0x02, 0xab, 0xcd, 0xef, 0x00, 0x11]));
        assert_eq!(sa.sa_family, libc::ARPHRD_ETHER as libc::sa_family_t);
        let raw: RawSockaddr = unsafe { std::mem::transmute(sa) };
        assert_eq!(&raw.data[..6], &[0x02, 0xab, 0xcd, 0xef, 0x00, 0x11]);
    }

    #[test]
    fn control_socket_owns_and_closes_its_descriptor() {
        use std::os::fd::AsRawFd;

        // AF_INET/SOCK_DGRAM needs no privilege, so creation must succeed unprivileged.
        let socket = control_socket()
            .unwrap_or_else(|error| panic!("control socket failed unexpectedly: {error}"));
        let raw = socket.as_raw_fd();
        assert!(unsafe { libc::fcntl(raw, libc::F_GETFD) } >= 0);

        drop(socket);

        assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    }
}
