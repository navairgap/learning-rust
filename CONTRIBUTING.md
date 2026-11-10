# Contributing

Thanks for your interest. This repository is a public learning record, so fixes to correctness, testing, and documentation are especially welcome.

## Getting started

The only project is [macabre](macabre/), a Linux CLI. A stable Rust toolchain is the only requirement:

```bash
cd macabre
cargo build
cargo test
```

CI runs the same gates on every push: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`. All four must pass before a PR is merged.

## Ground rules

- **Conventions** — the [README](README.md#conventions) defines the exit-code policy, the `unsafe` discipline, and where tests live. Changes should uphold them.
- **Tests** — behavior changes come with tests. CLI contract changes need a test in `macabre/tests/cli.rs`.
- **Commits** — Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`), matching the existing history.
- **Dependencies** — keep them minimal. macabre's only dependency is `libc`; a new one needs a justification in the PR description.
- **Platform** — macabre is Linux-only by design (ioctls, `/sys/class/net`). Portability layers are out of scope.

## Bugs and features

Open an issue with one of the templates (bug report or feature request). For ioctl problems, include the kernel version (`uname -r`) and the interface type. Small, well-scoped issues are labeled `good first issue` — start there if you're new to the codebase.

## Response time

I aim to respond to every issue and PR within 24 hours. If something has gone quiet, ping it — the notification got lost, not the interest.
