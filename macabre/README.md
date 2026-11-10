# macabre

Change and restore network interface MAC addresses on Linux. A single static-friendly binary with no daemon and no configuration — just ioctls and a JSON store.

## Motivation

A MAC address is visible to every network you join. Rotating it is a standard privacy practice; macabre makes it a single command and remembers the original so it can always be restored.

## Installation

```bash
cd macabre
cargo build --release
sudo cp target/release/macabre /usr/local/bin/
```

## Usage

```bash
macabre list                       # all interfaces + current MACs
macabre get wlp2s0                 # one interface's MAC
sudo macabre random wlp2s0         # random locally-administered MAC
sudo macabre set wlp2s0 02:ab:cd:ef:00:11
sudo macabre reset wlp2s0          # restore the saved original
macabre original wlp2s0            # show the saved original
macabre selftest                   # check entropy, interfaces, and store access
macabre --version
```

The first successful `set` or `random` saves the interface's prior MAC to `~/.local/share/macabre/originals.json` (or `$XDG_DATA_HOME/macabre/`). `reset` restores it and removes the entry. If the store cannot be written, the change is rolled back, so the tool never leaves an interface changed with no saved original to return to.

Exit codes: `0` success, `1` runtime failure, `2` usage error. Diagnostics go to stderr; command output goes to stdout.

## Design

- Interface listing reads `/sys/class/net`.
- MAC read/write uses `ioctl(SIOCGIFHWADDR / SIOCSIFHWADDR)` over a `SOCK_DGRAM/AF_INET` control socket — no external tools or `ip` subprocess, and read-only commands need no privileges.
- Saved MACs live in a small hand-parsed JSON store with no serde dependency. Writes are atomic: temp file, `fsync`, rename, directory `fsync`. Interface names containing JSON separators are escaped so the store round-trips.
- `unsafe` is confined to five audited blocks in `src/iface.rs` (socket creation, two ioctls, two sockaddr transmutes), each with checked returns.

## Safety notes

- Changing a MAC briefly drops connections on that interface; run it while idle.
- `random` generates only locally-administered unicast addresses (first octet `x2`/`x6`/`xA`/`xE`); multicast addresses would break switching.
- `set`/`random`/`reset` require root or `CAP_NET_ADMIN` (enforced by the kernel on the ioctl itself). `list`/`get`/`original`/`selftest` run unprivileged.

## Testing

```bash
cargo test
```

The suite covers MAC parsing and formatting, random-generation guarantees, the JSON store (including atomic writes and hostile interface names), ioctl argument validation, and the CLI contract (exit codes and diagnostics). All tests run unprivileged.
