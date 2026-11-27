# Development toolchain

[简体中文](../zh-CN/architecture/DEVELOPMENT_TOOLCHAIN.md) · [Documentation index](../INDEX.md)

PR 001 establishes a Rust workspace, a pnpm workspace, and a shared developer entry point for Linux and Windows.

## Approved toolchain

| Tool | Version | Declaration | Purpose |
|---|---|---|---|
| Rust and Cargo | 1.100.0 | [rust-toolchain.toml](../../rust-toolchain.toml) | Compile the Rust workspace and its developer tool. |
| Node.js | 24.22.3 LTS | [package.json](../../package.json) | Run the JavaScript workspace tooling. |
| pnpm | 11.25.0 | [package.json](../../package.json) | Resolve and lock JavaScript workspace packages. |

Use the official distributions and put the matching tools on PATH. Rustup reads the toolchain pin from the checkout. A host linker also needs to be available for Rust. Rust and Cargo are offered under Apache-2.0 or MIT, Node.js uses MIT with bundled-component notices, and pnpm uses MIT. The approved Node version is a security release and satisfies pnpm's minimum Node version of 22.13. Upstream sources are the [Rust release](https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/), [Node.js release](https://nodejs.org/en/blog/release/v24.22.3), and [pnpm registry metadata](https://registry.npmjs.org/pnpm/11.25.0).

## Commands

Run these commands from the repository or a nested project directory.

| Command | Effect |
|---|---|
| `cargo dev doctor` | Locate the workspace and verify the active Rust, Cargo, Node.js and pnpm versions against their project pins. |
| `cargo dev init` | Perform preflight, fetch locked Rust dependencies, and install the locked pnpm workspace. |
| `cargo dev build` | Build the Rust workspace with the checked-in lockfile. |
| `cargo dev verify` | Run Cargo workspace tests and verify frozen pnpm lockfile resolution. |

The Rust workspace uses [Cargo.toml](../../Cargo.toml) and [Cargo.lock](../../Cargo.lock). The pnpm workspace uses [pnpm-workspace.yaml](../../pnpm-workspace.yaml) and [pnpm-lock.yaml](../../pnpm-lock.yaml). Its package paths cover apps and shared frontend packages as those projects are added. Each ecosystem has one root lockfile. Tooling resolves subprocesses from the project root and uses the available host toolchain. Cargo uses its workspace target directory by default. pnpm's store and cache are under `.winstrata/`. Generated output, installs and local caches are ignored.

## Acceptance scope

PR 001 builds and exercises the developer tool on the available host. Its Rust crate uses the standard library, and the JavaScript workspace has a root package with zero dependencies. Linux evidence is recorded with the [project progress](../records/STATUS.md). Windows host verification and Windows x64 application and driver cross-compilation remain scheduled under the [roadmap](../roadmap/PLAN.md). The shared quality gates are planned for PR 002.
