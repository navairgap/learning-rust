# macabre

**your MAC, requiem'd.**

change and restore network interface MAC addresses on Linux. one small binary,
no daemon, no config language — just ioctls and a JSON file.

## why

your MAC address is visible to every network you join. cafes, hotels, and
airport wifi log it. changing it is a standard privacy practice — macabre
makes it one command, and remembers the original so you can put it back.

## install

```bash
cd macabre
cargo build --release
sudo cp target/release/macabre /usr/local/bin/
```

## usage

```bash
macabre list                       # all interfaces + current MACs
macabre get wlp2s0                 # one interface's MAC
sudo macabre random wlp2s0         # random locally-administered MAC
sudo macabre set wlp2s0 02:ab:cd:ef:00:11
sudo macabre reset wlp2s0          # restore the saved original
macabre original wlp2s0            # show the saved original
```

the first `set` or `random` on an interface saves its current MAC to
`~/.local/share/macabre/originals.json`. `reset` restores it and clears
the entry.

## how it works

- interface listing reads `/sys/class/net`
- MAC read/write uses `ioctl(SIOCGIFHWADDR / SIOCSIFHWADDR)` through a
  `SOCK_DGRAM/AF_PACKET` control socket — no external tools, no `ip` subprocess
- original MACs persist in a small JSON store (no serde — the format is
  simple enough to parse by hand, and one less dependency)

## safety notes

- changing a MAC drops connections on that interface briefly; do it while idle
- `random` always generates locally-administered, unicast addresses
  (first octet `x2`/`x6`/`xA`/`xE`) — multicast addresses would confuse switches
- run tests without root; the tool itself needs root or `CAP_NET_ADMIN`

## tests

```bash
cargo test
```

MAC parsing, formatting, random-generation guarantees, the JSON store, and
ioctl argument validation are all covered and run unprivileged in CI.

## license

MIT
