# learning-rust

Rust exercises and mini-projects from my learning journey

> 🚧 **Status: active exercises** — the repository includes the `macabre` Linux CLI and its tested modules; additional exercises are planned.

## Why

Learning Rust in public: exercises, small tools, and the mistakes that taught me. From ownership battles to async runtime internals, tracked commit by commit.

## Planned features

- Rustlings-style exercises with my annotated solutions
- Mini-projects: CLI tool, web server, a parser combinator
- Notes on ownership, lifetimes, traits — the hard parts
- A public learning log in the README

## Stack

`rust` `cargo`

## Notes

Slow is smooth: each concept gets its own branch and a real mini-project.

## License

MIT, see [LICENSE](LICENSE).


## How to use this repo

each top-level directory is one exercise, ordered roughly by difficulty. read the `README.md` inside each, implement it, then compare with the reference in `solutions/`. resist the urge to peek first — the compiler is a better teacher than the answer key.

## projects

- **macabre** — change and restore interface MAC addresses on Linux (ioctls, zero-dep JSON store, tested). see [macabre/](macabre/).

## Reading macabre

start at `src/main.rs` (what the binary does), then `src/mac.rs` (the address type — pure, fully tested), then `src/iface.rs` (the linux ioctl layer). each module has exactly one responsibility; tests live beside the code they cover.

## Roadmap

next rust projects, in order: a tiny shell (fork/exec/pipes), a bump allocator, a modal text editor. each reuses something macabre taught — ioctls, unsafe discipline, or argument parsing without clap.

## Unsafe audit

macabre has five `unsafe` blocks: socket creation, two ioctls, and the two sockaddr transmutes. each wraps a libc call with checked returns; no unsafe leaks past the iface boundary.

## Error handling policy

library code returns `io::Result`; only `main` decides exit codes. no panics — even the entropy source failure is a normal error. the store tolerates corrupt files by starting empty.

## Testing strategy

unit tests live beside the code in `#[cfg(test)]` modules plus a CLI contract suite in `tests/`; CI runs build + test + clippy + rustfmt on every push. the `selftest` subcommand is an end-to-end check a human can run after any change.

## Toolchain

stable rustc only, no nightly features. if stable can't do it yet, the project waits.

## History

rust started as the answer to 'what after python?' — the honest answer turned out to be: the same discipline, with a borrow checker. macabre came first; sentinel-dns proved the pattern scales.
