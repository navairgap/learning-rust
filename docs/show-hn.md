# Show HN draft

Post after the v0.1.0 release is live and the demo GIF is on the README. Best window: Tuesday–Thursday, 8–10 am US Eastern. Reply to every comment in the first few hours.

## Title

Show HN: Macabre – Change and restore your MAC address from a single Linux CLI

## Text

macabre is a small Rust CLI that changes a network interface's MAC address and remembers the original so you can always put it back. Your hardware address is visible to every network you join, and rotating it is standard privacy practice — but doing it by hand means `ip link` incantations and hoping you wrote the old address down. macabre makes rotation one command (`sudo macabre random wlan0`), and `sudo macabre reset wlan0` always restores what was there before.

It's about 1,400 lines of Rust with one dependency: libc. It talks to the kernel directly over the SIOCGIFHWADDR/SIOCSIFHWADDR ioctls — no `ip` subprocess, no daemon, no config file. For the Rust-curious, the parts I'd highlight: five audited `unsafe` blocks confined to a single module, a hand-written JSON store with atomic writes and rollback, and a CLI with a strict exit-code policy (0 success / 1 runtime / 2 usage) covered by contract tests.

The design decision I'd most like feedback on is the original-store rollback: the first successful change saves the prior MAC, and if the store can't be written the change is rolled back, so an interface never ends up changed with no saved original to return to.

Repo: https://github.com/navairgap/learning-rust — macabre lives in `macabre/` and is a single `cargo install` away.

## Expected questions, pre-drafted

- **Why not macchanger?** Scope: macabre does one job — rotate and restore — with no flags to memorize, and it guarantees a path back to the original.
- **Does this defeat tracking?** No. MAC randomization is built into modern Wi-Fi tooling (iwd, NetworkManager); macabre covers the cases where you control the interface directly — wired, VMs, containers, scripting.
- **Windows/macOS?** Linux-only by design; the ioctl layer is the point of the project.
