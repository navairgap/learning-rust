# Changelog

All notable changes to the projects in this repository are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-11-10

### Added

- **macabre** — a Linux CLI that changes and restores network interface MAC addresses (`list`, `get`, `set`, `random`, `reset`, `original`, `selftest`).
- Random locally-administered unicast MAC generation (first octet `x2`/`x6`/`xA`/`xE`).
- Original-MAC store at `~/.local/share/macabre/originals.json` with atomic writes, and rollback when the store cannot be written, so an interface is never left changed with no saved original.
- Unprivileged read commands (`list`, `get`, `original`, `selftest`); write commands require root or `CAP_NET_ADMIN`, enforced by the kernel on the ioctl.
- Distinct exit codes: `0` success, `1` runtime failure, `2` usage error. Diagnostics on stderr, data on stdout.
- Unit tests for MAC parsing, generation, and store round-trips; CLI contract tests; CI running build, tests, clippy `-D warnings`, and rustfmt on every push.

[0.1.0]: https://github.com/navairgap/learning-rust/releases/tag/v0.1.0
