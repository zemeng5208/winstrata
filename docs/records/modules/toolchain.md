# Toolchain error register

[简体中文](../../zh-CN/records/modules/toolchain.md) · [Global register](../ERRORS.md)

Authoritative error records for `toolchains/dev` and the pinned host toolchain.

<a id="err-002"></a>

## ERR-002

- Error ID: ERR-002
- Discovery time: 2026-11-27, plan 001 preflight. A clock time was not recorded.
- Related PR: GitHub PR #2, plan 001
- Environment: Linux x86_64 development host before the approved toolchain was installed
- Reproduction method: `cargo dev doctor` with Rust and Cargo absent from `PATH`
- Evidence: preflight exited because Rust and Cargo were missing. The official toolchain was installed into the project's ignored local tooling directory after the Rustup installer checksum was verified. Both version checks and the build then passed.
- Root cause: the host did not have the approved Rust toolchain on `PATH`
- Impact: plan 001 preflight could not build until the toolchain was installed
- Fix plan: install the pinned toolchain locally. Done in that environment.
- Retest result: `cargo dev doctor`, `cargo dev init`, `cargo dev build`, and `cargo dev verify` passed with Rust and Cargo 1.100.0
- Current state: resolved

Later hosts that had Node.js v22.23.4 instead of the pinned 24.22.3 did not run `cargo dev doctor` locally. That gap is [OI-010](../OPEN_ITEMS.md#oi-010). It is not this resolved error.
