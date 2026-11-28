# Platform Implementation Record and Build Isolation Standard

[简体中文](../zh-CN/standards/PLATFORM_ISOLATION.md) · [Index](../INDEX.md) · [AG-65](../../AGENTS.md#ag-65)

- **Document status**: Authoritative standard for [AG-65](../../AGENTS.md#ag-65), effective upon owner approval and merge of Plan 055.
- **Recorded code baseline**: Initially recorded at `4f257aee948d10af41e7fbddb96d8cfaa46697af` (`main` after PR 030 merge); updated in Plan 054 on top of PR 055 (`main` after Plan 055 merge) for Windows 11 AppContainer runtime isolation, WebView2/tray fixes, and verification scope. Any PR changing platform code, `cfg` gates, target dependencies, build/packaging scripts, or platform test coverage MUST update this document in the same PR.
- **Non-runtime artifact rule**: This document and all `docs/` standards are **source-repository engineering documentation only**. They MUST NOT be bundled or shipped as runtime resources in any final software installer, portable package, or runtime directory.
- **Current product scope**: WinStrata V1.0 targets **Windows 10 and Windows 11 x64**. Existing Linux implementations in this repository are **development-phase technical reserves** used to sustain dual-environment build/test coverage under [AG-35](DEPENDENCIES.md#ag-35) and [AG-49](GIT_AND_PR.md#ag-49). They do NOT constitute official Linux product support, MUST NOT be deleted, and MUST NOT trigger unapproved refactoring or scope expansion.

---

<a id="section-1"></a>

## 1. Current Platform Implementation Record

This section records the actual Windows and Linux platform-specific implementations verified against baseline `4f257ae` and updated through Plan 054.

### 1.1 Core Infrastructure, Kernel, Services, and Protocols

| Module / Crate | Inspected Paths | Windows Implementation | Linux Technical Reserve | Reuse Value (Future Linux) | Verified Status & Open Items |
|---|---|---|---|---|---|
| Core platform & ACLs (`winstrata-core`) | `core/src/platform/mod.rs`, `linux.rs`, `windows.rs`, `windows/security.rs`, `windows/tests.rs` | `NtCreateFile` (`OBJ_DONT_REPARSE`), `CreateFileW`, private DACL/SDDL ownership, `SHGetKnownFolderPath` (`LocalAppData`), `CoCreateGuid`, `GetDiskFreeSpaceExW`, `GetVolumeInformationByHandleW`. | `openat` (`O_CLOEXEC \| O_DIRECTORY \| O_NOFOLLOW \| O_PATH`), `mkdirat` (`0o700`/`0o755`), `fstatvfs`, `/dev/urandom`, `geteuid`, `XDG_STATE_HOME` / `$HOME/.local/state`. | **High**: Path-pinning and symlink/reparse defenses mirror Windows semantics directly. | Linux & Server 2022 tests pass; Windows 10/11 clients ([OI-001](../records/OPEN_ITEMS.md#oi-001)) & ReFS ([OI-002](../records/OPEN_ITEMS.md#oi-002)) remain open. |
| Core cryptography (`winstrata-core`) | `core/src/crypto/mod.rs`, `openssl.rs`, `cng.rs` | Windows CNG (`BCrypt*`) for AES-256-GCM, PBKDF2-HMAC-SHA256, and RNG. | Dynamic FFI link to `libcrypto.so.3` (OpenSSL 3 `EVP_*`, `PKCS5_PBKDF2_HMAC`, `HMAC`). | **High**: Both backends pass the same published test vectors. | Vector tests pass on both hosts; key rotation crash-safety ([OI-008](../records/OPEN_ITEMS.md#oi-008)) & Linux distro OpenSSL ABI packaging unverified. |
| Core files, storage, scope, migration & hybrid (`winstrata-core`) | `core/src/files.rs`, `storage.rs`, `scope.rs`, `migration.rs`, `hybrid/mod.rs`, `hybrid/tests.rs` | `GetFileInformationByHandle`, `FILE_FLAG_OPEN_REPARSE_POINT`, `CompareStringOrdinal`, `GetShortPathNameW`, NTFS stream checks. | `O_NOFOLLOW \| O_NONBLOCK`, `0o600` private files, `dev`/`ino` checks, POSIX path equality. | **High**: Shared storage/migration formats and safety boundaries work natively on Linux. | Linux storage/migration tests pass; ReFS ([OI-002](../records/OPEN_ITEMS.md#oi-002)), second-device migration, and [E2E-10](../roadmap/ACCEPTANCE.md#e2e-10) remain open. |
| Core file/registry watch (`winstrata-core`, `winstrata-kernel`) | `core/src/file_registry/mod.rs`, `windows.rs`, `kernel/src/file_registry.rs` | User-mode `ReadDirectoryChangesW` & `RegNotifyChangeKeyValue` on-demand watches (`strata-fs.sys`/`strata-reg.sys` not loaded). | `CollectHost::start` returns `CollectError::Unsupported`; `stop` returns `Ok(())`. | **Medium**: Refcount & health state machine in `kernel/src/file_registry.rs` is reusable; Linux `inotify`/`fanotify` watch is not implemented. | Windows watch tests pass on Server 2022; signed kernel drivers & [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) remain open ([OI-007](../records/OPEN_ITEMS.md#oi-007)). |
| Core & kernel process/system (`winstrata-core`, `winstrata-kernel`) | `core/src/process.rs`, `schedule.rs`, `kernel/src/system.rs`, `system/linux.rs`, `system/windows.rs` | `Toolhelp` snapshot + SCM service PID correlation. | `/proc` scan + `/etc/machine-id` + `/proc/sys/kernel/random/boot_id` + `/proc/stat` `btime` + `/proc/<pid>/cgroup` `.service` units. | **High**: Process tree, PID-reuse `start_unix_ns` dedup, and cgroup unit mapping are already working on Linux. | Process-exit sample (`ERR-005`) resolved on Server 2022; signed `strata-proc.sys`, ETW, and command lines remain open ([OI-015](../records/OPEN_ITEMS.md#oi-015)). |
| Kernel network (`winstrata-kernel`, `winstrata-core`) | `kernel/src/net/mod.rs`, `linux.rs`, `windows.rs`, `parse.rs`, `core/src/network.rs` | IP Helper tables + WinSock raw socket capture (memory-only unless tagged). | `AF_PACKET` `SOCK_RAW` + `/proc/net` + sysfs interfaces (records `capture_inaccessible` gap without `CAP_NET_RAW`). | **High**: Shared protocol parser and memory-only policy; Linux raw capture works when permitted. | Loopback/interface tests pass; `strata.net` plugin ([PR 031](../roadmap/PLAN.md#pr-031)), signed `strata-net.sys`, and [E2E-04](../roadmap/ACCEPTANCE.md#e2e-04) remain open. |
| Service control & IPC (`winstrata-kernel`, `winstrata-service`) | `kernel/src/scm.rs`, `services/src/transport.rs`, `transport/linux.rs`, `transport/windows.rs`, `services/src/tests.rs` | Windows SCM probe/enumeration + Named Pipe with `SECURITY_IMPERSONATION` and restricted-token denial. | Unix Domain Socket under `$XDG_RUNTIME_DIR/winstrata/service.sock` (`0o700`), `Privilege::NotWindows` (no driver control). | **Medium**: Framed IPC protocol, audit log, and refcount manager are reusable; Linux privileged daemon (e.g., systemd/polkit/capabilities) is unimplemented. | Linux socket & Server 2022 pipe tests pass; real SCM registration, driver load ([OI-007](../records/OPEN_ITEMS.md#oi-007)), and [E2E-14](../roadmap/ACCEPTANCE.md#e2e-14) remain open. |
| Protocol contracts (`winstrata-protocols`) | `protocols/Cargo.toml`, `protocols/src/*`, `protocols/tests/windows_event.rs`, `interop.rs` | Cross-platform JSON/frame contracts + live Windows Event Log query test (`windows_event.rs`). | Cross-platform Rust/Node interop tests pass on Linux. | **High**: Fully reusable across platforms. | Verified on both Linux and Server 2022. |

### 1.2 Official Plugins and Host Integrations (`plugins/strata-*`, `core/src/plugins/`)

| Plugin / Module | Inspected Paths | Windows Implementation | Linux Technical Reserve | Reuse Value (Future Linux) | Verified Status & Open Items |
|---|---|---|---|---|---|
| `strata.process` (`winstrata-strata-process`) | `plugins/strata-process/src/*`, `tests/observe.rs`, `core/src/plugins/process.rs` | Out-of-process collector using `winstrata_kernel::system::capture()`. | Same binary delegates to `kernel/src/system/linux.rs` (`procfs`); `#[cfg(unix)]` process env clear & session pipe helpers. | **High**: Real Linux child lifecycle & thread/module/tree tracking already implemented. | Linux & Server 2022 tests pass; ETW/command lines/Windows 10–11 remain open ([OI-015](../records/OPEN_ITEMS.md#oi-015), [OI-001](../records/OPEN_ITEMS.md#oi-001)). |
| `strata.registry` (`winstrata-strata-registry`) | `plugins/strata-registry/src/lib.rs`, `windows.rs`, `core/src/plugins/registry.rs` | In-process `RegNotifyChangeKeyValue` watch for up to 4 HKCU/HKLM keys. | Returns `RegistryError::Unsupported` / `unsupported` without emitting events. | **Low (API-specific)**: Tracker/event model reusable; Windows Registry itself does not exist on Linux (future Linux config stores require separate design). | Server 2022 tests pass (local non-admin HKLM write fails with Win32 error 5); [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) open. |
| `strata.env` (`winstrata-strata-env`) | `plugins/strata-env/src/lib.rs`, `windows.rs`, `core/src/plugins/env.rs` | In-process HKCU/HKLM environment & PATH diff watcher (`windows.env_notify`). | Returns `EnvError::Unsupported` / `unsupported` without emitting events. | **Medium**: Diff/PATH-segment encoding reusable; Linux `/etc/environment`, shell profiles, and systemd user env watchers are not implemented. | Server 2022 tests pass (local non-admin HKLM open fails with Win32 error 5); [OI-039](../records/OPEN_ITEMS.md#oi-039), [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) open. |
| `strata.trace` (`winstrata-strata-trace`) | `plugins/strata-trace/src/lib.rs`, `windows.rs`, `core/src/plugins/trace.rs` | In-process `ReadDirectoryChangesW` metadata/SDDL watcher + baseline link projection (`winstrata.trace`). | Returns `TraceError::Unsupported` / `unsupported` without emitting events. | **Medium**: Projection/history-linking logic is reusable; Linux `fanotify`/`inotify` collector is unimplemented. | Server 2022 (`ERR-019` resolved) & Linux projection tests pass; `strata-fs.sys` & [E2E-02](../roadmap/ACCEPTANCE.md#e2e-02) open. |
| `strata.services` (`winstrata-strata-services`) | `plugins/strata-services/src/lib.rs`, `windows.rs`, `linux.rs`, `session.rs`, `tests/scm.rs` | `EnumServicesStatusExW` + `QueryServiceConfigW` SCM read-only snapshot & restricted-token check. | `linux.rs` scans `/proc/*/cgroup` `.service` units and `/proc/modules` (`Coverage::ActiveOnly`, `evidence = "procfs"`). | **Medium–High**: Diff/tracker state machine reusable; Linux inactive unit files and non-systemd init systems are not covered. | `ERR-020` resolved; **`ERR-021` remains open** (`service.start-type` count 0 on Windows); [OI-043](../records/OPEN_ITEMS.md#oi-043) open. |
| `strata.startup` (`winstrata-strata-startup`) | `plugins/strata-startup/src/lib.rs`, `windows.rs`, `linux.rs`, `parse.rs`, `tests/probes.rs` | Run/RunOnce/PolicyRun registry keys, Startup folders, StartupApproved, Task Scheduler XML. | `linux.rs` + `parse.rs` parse XDG autostart `.desktop`, `/etc/crontab`, `/etc/cron.d`, `/etc/cron.*`, user crontabs, and systemd system/user `.timer` units. | **High**: Real Linux startup/cron/timer parsers already implemented and unit/integration tested. | Linux tests pass; Server 2022 probe recorded ([OI-049](../records/OPEN_ITEMS.md#oi-049)), Windows 10/11 client checks remain open. |
| `strata.pulse` (`winstrata-strata-pulse`) | `plugins/strata-pulse/src/platform.rs`, `linux.rs`, `windows.rs`, `sample.rs` | `GetSystemTimes`, `GlobalMemoryStatusEx`, `CallNtPowerInformation`, `GetSystemPowerStatus`, `ToolHelp`, `GetProcessMemoryInfo` (GPU counters marked `reader_absent`). | `linux.rs` reads `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`, sysfs CPU/GPU/battery/sensors, RAPL, and `perf_event_open`. | **Very High**: Rich native Linux telemetry backend already implemented. | Linux & Server 2022 unit/observe tests pass; Windows load scenario ([OI-046](../records/OPEN_ITEMS.md#oi-046)) & GPU/disk remain open. |
| `strata.budget` (`winstrata-strata-budget`) | `plugins/strata-budget/src/regulate.rs`, `tests/regulate.rs`, `core/src/plugins/budget.rs` | `GetProcessTimes` & `GetProcessMemoryInfo` in Core budget monitor; cross-platform plugin regulation loop. | `/proc/self/stat`, `/proc/self/status` (`VmRSS`), `sysconf(_SC_CLK_TCK)`, `0o600` private output files (`#[cfg(unix)]`). | **High**: Regulation & measurement contract works on both platforms. | Linux tests pass; Windows budget tests ([OI-050](../records/OPEN_ITEMS.md#oi-050)) & disk/network/GUI refresh budgets ([OI-051](../records/OPEN_ITEMS.md#oi-051)) open. |
| `strata.script` (`winstrata-strata-script`) | `plugins/strata-script/src/contain.rs`, `execute.rs`, `workbench.rs`, `tests/workbench.rs` | Spawns Python/JS/TS child processes; **Windows AppContainer containment for script children is not implemented here** ([OI-045](../records/OPEN_ITEMS.md#oi-045)). | `contain.rs` builds Landlock ruleset (ABI 1–5) + `LD_PRELOAD` `winstrata_shim`; clears inherited env (`#[cfg(unix)]`). | **High**: Linux script containment is already enforced. | Linux & Server 2022 workbench tests pass (`ERR-022`, `ERR-024` resolved); [OI-044](../records/OPEN_ITEMS.md#oi-044) & [OI-045](../records/OPEN_ITEMS.md#oi-045) open. |
| `strata.storage` (`winstrata-strata-storage`) | `plugins/strata-storage/src/lib.rs`, `windows.rs`, `tests/cleanup.rs` | `windows.rs` writes private alert/config files with explicit owner & protected DACL. | `#[cfg(unix)]` opens private files with `mode(0o600)`. | **High**: Threshold ladder & index drop logic are platform-independent. | `ERR-025` & `OI-058` closed for specific Server 2022 head; [OI-024](../records/OPEN_ITEMS.md#oi-024), [OI-057](../records/OPEN_ITEMS.md#oi-057), [OI-059](../records/OPEN_ITEMS.md#oi-059), [OI-060](../records/OPEN_ITEMS.md#oi-060) open. |
| `strata.filter`, `strata.tag`, `strata.health` | `plugins/strata-filter/`, `plugins/strata-tag/`, `plugins/strata-health/` | Cross-platform protocol & state logic; Windows CI verifies compilation and integration tests. | `#[cfg(unix)]` helpers in `session.rs` / `report.rs` / `tests/` (FD/pipe/process helpers). | **High**: Business logic is already platform-neutral. | Server 2022 filter/tag/health tests pass on respective review heads; GUI pages & MCP integration remain open ([OI-053](../records/OPEN_ITEMS.md#oi-053), [OI-054](../records/OPEN_ITEMS.md#oi-054), [OI-056](../records/OPEN_ITEMS.md#oi-056), [OI-047](../records/OPEN_ITEMS.md#oi-047)). |

### 1.3 Language Runtimes, Desktop GUI, and Toolchains

| Area | Inspected Paths | Windows Implementation | Linux Technical Reserve | Reuse Value (Future Linux) | Verified Status & Open Items |
|---|---|---|---|---|---|
| Language runtime isolation & shim (`winstrata-core`, `winstrata-runtime`, `winstrata-shim`) | `core/src/plugins/language.rs`, `sandbox.rs`, `runtimes/Cargo.toml`, `runtimes/shim/src/lib.rs`, `runtimes/tests/languages*.rs` | AppContainer profile + Job Object (`kill-on-close`, single active process); `Toolchain::bind` resolves `sys._base_executable`; non-inheritable top-level/stdlib prefix DACL grants (`grant_python_prefix`, `grant_node_prefix`), `SE_DACL_PROTECTED` descendant ACEs, and `revoke_all` cleanup; `winstrata-shim` compiles empty (`#[cfg(target_os = "linux")]` only) and is not loaded. | Landlock ruleset + `LD_PRELOAD` `libwinstrata_shim.so` + seccomp-bpf (`AUDIT_ARCH_X86_64`, denies sockets/`execve`/`fork`/mount/namespaces) + `close_range`. | **High**: Real Linux containment already passes `languages.rs` on x86_64 Linux. | Linux containment verified; non-x86_64 Linux arches unimplemented (`fail(b'x')`); Plan 054 fixed [ERR-026](../records/modules/core.md#err-026) and verified `fixture.pywin` + pinned runtime check on Windows 11 (`languages_windows` 2/2); Windows `node.exe` hits `0xC0000142` under restricted desktop DACLs ([ERR-028](../records/modules/core.md#err-028), [OI-019](../records/OPEN_ITEMS.md#oi-019), [OI-062](../records/OPEN_ITEMS.md#oi-062)); [OI-020](../records/OPEN_ITEMS.md#oi-020) & `runtimes/Cargo.toml` unconditional dev-dep `libc` noted. |
| Official Desktop GUI host (`winstrata-desktop`, `packages/ui`) | `apps/desktop/src/main.rs`, `lib.rs`, `host.rs`, `bridge.rs`, `prefs.rs`, `tests/official_gui.rs`, `packages/ui/*` | Win32 window + system tray + WebView2 (`1.0.2903.40` ABI, AMD64 PE loader validation + `vswhere.exe` `CREATE_NO_WINDOW` discovery, `ICoreWebView2Controller` COM lifetime + `WM_SIZE` + `WM_PAINT` fallback + pending storage tray tooltip) + `BCryptGenRandom` + `LocalAppData` prefs. | Localhost HTTP bridge + `XDG_STATE_HOME` prefs + `/dev/urandom`; `main.rs` waits on stdin with **no native Linux window or WebView host**. | **Medium**: Bridge, preferences schema, and `packages/ui/dist` bundle are reusable; Linux desktop window/tray/webview shell is unimplemented. | Linux headless Chrome bridge checks pass; Plan 054 fixed [ERR-027](../records/modules/desktop.md#err-027), closed [OI-013](../records/OPEN_ITEMS.md#oi-013) & [OI-057](../records/OPEN_ITEMS.md#oi-057) on Windows 11 ordinary user (`winstrata-desktop` 11/11 + Edge WebView2 `158.0.4370.45`); Windows 10 ([OI-001](../records/OPEN_ITEMS.md#oi-001)), [OI-009](../records/OPEN_ITEMS.md#oi-009), and [E2E-11](../roadmap/ACCEPTANCE.md#e2e-11) remain open. |
| Dev CLI & cross-build (`winstrata-dev`, `.github/`) | `toolchains/dev/src/main.rs`, `cross_probe.rs`, `toolchains/windows/*`, `.github/scripts/*`, `.github/workflows/*` | Windows CI runs `cargo test --locked --workspace`; `cargo dev build` hits Windows binary self-lock (`os error 5`) if `winstrata-dev.exe` is rebuilt while running. | `cargo dev cross-probe` links PE32+ console & kernel `.sys` probes via `rust-lld` + `objdump`; Linux CI runs `check-workspace.sh`. | **High**: Unified toolchain pins (`Rust 1.100.0`, `Node 24.22.3`, `pnpm 11.25.0`). | Linux `cross-probe` & CI pass; full Linux-hosted Windows app/driver cross-build & Windows `cargo dev` runs remain open ([OI-010](../records/OPEN_ITEMS.md#oi-010), [OI-011](../records/OPEN_ITEMS.md#oi-011), [OI-012](../records/OPEN_ITEMS.md#oi-012)). |

### 1.4 Platform-Gated Dependencies Summary (`Cargo.toml`)

- **Windows-only (`[target.'cfg(windows)'.dependencies]` / `dev-dependencies`)**: `windows-sys = "=0.61.2"` in `apps/desktop`, `core`, `kernel`, `services`, `plugins/strata-env`, `plugins/strata-pulse`, `plugins/strata-registry`, `plugins/strata-services`, `plugins/strata-startup`, `plugins/strata-storage`, and `plugins/strata-trace`.
- **Linux / Unix-only (`[target.'cfg(target_os = "linux")'.dependencies]` or `cfg(unix)`)**: `libc = "=0.2.191"` in `core` (`linux`), `kernel` (`linux`), `services` (`linux`), `plugins/strata-pulse` (`linux`), `apps/desktop` (`unix`), `plugins/strata-script` (`unix`), and `plugins/strata-startup` (`unix`); dynamic `libcrypto.so.3` in `core/src/crypto/openssl.rs` (`linux`).
- **Observed hygiene item**: `runtimes/Cargo.toml` currently declares `libc.workspace = true` unconditionally under `[dev-dependencies]` (used by `#![cfg(unix)]` test `runtimes/tests/languages.rs`). It does not affect release binaries, and when touched by a relevant runtime/packaging cleanup it should move under `[target.'cfg(unix)'.dev-dependencies]`.

---

<a id="section-2"></a>

## 2. Build and Packaging Isolation Requirements

All future compilation, linking, dependency resolution, packaging, and release verification MUST enforce strict target-platform isolation:

1. **Separate Target Compilation, Linking, and Packaging**:
   - Windows and Linux artifacts MUST be compiled, linked, dependency-resolved, and packaged separately for their explicit target triple (e.g., `x86_64-pc-windows-msvc` vs future Linux targets).
2. **Zero Cross-Platform Contamination in Deliverables**:
   - A platform's final deliverable package MUST NOT include another platform's dead code, binaries, shared libraries, runtime shims, dependencies, or assets:
     - **Windows packages** MUST NOT ship `libwinstrata_shim.so`, ELF binaries, `libc` or `libcrypto.so.3` dependencies, Linux-only scripts/units, or source-repo engineering documentation (`docs/`, `AGENTS.md`, `PLATFORM_ISOLATION.md`).
     - **Future Linux packages** MUST NOT ship `.exe`, `.dll`, `.sys`, `.inf`, `.cat` files, `windows-sys` / WebView2 / WDK artifacts, or Windows-only collector resources.
3. **Zero-Overhead Shared Logic**:
   - Platform-neutral logic (`winstrata-protocols`, bounded scheduling, event bus, storage formats, shared tracker state machines) may be shared only when doing so adds no extra runtime overhead, unnecessary abstraction layers, or extra background services/processes.
4. **Mandatory Deliverable Verification**:
   - Before final delivery ([PR 050](../roadmap/PLAN.md#pr-050) and any future Linux release), isolation MUST be verified with concrete evidence:
     - **Artifact manifest audit**: Exact file list of the deliverable against a platform allowlist (confirming docs/tests/foreign assets are excluded).
     - **Dependency graph check**: `cargo tree --target <target> -e normal` confirming no foreign platform crates (`libc`/`openssl` on Windows; `windows-sys` on Linux) appear in production dependency graphs.
     - **Binary & import inspection**: PE32+ import/header inspection (`objdump`/`dumpbin`) on Windows deliverables and `ldd`/`readelf` on Linux deliverables.
     - **Runtime performance & size check**: Measurement against [ACCEPTANCE.md §10](../roadmap/ACCEPTANCE.md#section-10) targets (artifact size <= 250 MB, background memory <= 150 MB, CPU <= 1%, GUI memory <= 350 MB, cold start <= 3 s).
5. **Machine-Verifiable Tooling Enforcement**:
   - Build and delivery tooling (`toolchains/dev`, `.github/scripts/`, and release packaging scripts) MUST implement automated, machine-verifiable isolation gates (target-dependency assertions, artifact allowlist & foreign-binary scans, non-runtime doc exclusion checks) rather than relying solely on Agent compliance. Tracked as [OI-061](../records/OPEN_ITEMS.md#oi-061) for Plan 050.

---

<a id="section-3"></a>

## 3. Mandatory Active Reading and Traceability Mechanism

Under [AG-65](../../AGENTS.md#ag-65), the root [AGENTS.md](../../AGENTS.md) and [docs/zh-CN/AGENTS.md](../zh-CN/AGENTS.md) establish a **mandatory active-reading gate** (not merely an optional reference link).

### 3.1 Mandatory Trigger Conditions

All coordinator Agents, development Agents, and subagents MUST actively read the latest committed `PLATFORM_ISOLATION.md` **before starting** any task that involves:

1. Developing or modifying platform-specific functionality;
2. Modifying platform interfaces, shared/public interfaces, or platform dependencies;
3. Compiling, cross-compiling, or building;
4. Packaging, releasing, or final delivery;
5. Officially starting future Linux version development;
6. Platform compatibility checks or performance optimization.

### 3.2 Mandatory Compliance Rules

- **No passive waiting**: Agents MUST NOT wait for the repository owner to remind them to read this document.
- **No stale memory**: Agents MUST NOT rely on conversation history, compacted summaries, or Agent temporary memory in place of reading the current file on disk / latest `main`.
- **Mandatory version citation**: Each time a triggered task starts, the Agent MUST read the current version of `PLATFORM_ISOLATION.md` and record the exact document commit SHA or baseline revision cited in its engineering record ([STATUS.md](../records/STATUS.md), module record, handover, or PR body).
- **Automated gate backing**: Machine-verifiable isolation rules belong in build/delivery scripts ([§2.5](#section-2)) so non-compliant artifacts fail automatically.

---

<a id="section-4"></a>

## 4. Long-Term Maintenance and Future Linux Bootstrap

1. **Synchronized Updates**: Whenever a PR adds, removes, renames, or changes platform-specific code, `cfg` gates, target dependencies, build/packaging pipelines, or platform verification status, the PR MUST update both `docs/standards/PLATFORM_ISOLATION.md` and `docs/zh-CN/standards/PLATFORM_ISOLATION.md` in the same PR ([AG-06](DOCUMENTATION.md#ag-06), [AG-65](../../AGENTS.md#ag-65)).
2. **Future Linux Development Protocol**: When official Linux product development starts, Agents MUST:
   - Read this document first;
   - Audit every existing Linux implementation listed in [§1](#section-1) against the then-current `main` branch;
   - Reuse verified Linux modules (e.g., `core/src/platform/linux.rs`, `core/src/crypto/openssl.rs`, `kernel/src/system/linux.rs`, `kernel/src/net/linux.rs`, `runtimes/shim`, `plugins/strata-pulse/src/linux.rs`, `plugins/strata-startup/src/linux.rs`) to avoid duplicate implementation;
   - Re-verify any stale or partial implementation on real target Linux environments before enabling it—never assume a development-phase reserve is production-ready without fresh verification.
3. **Remote Traceability**: This document and its root `AGENTS.md` mandatory-reading entries MUST live on `main` in the remote GitHub repository via approved PRs ([AG-12](GIT_AND_PR.md#ag-12), [AG-60](../../AGENTS.md#ag-60)) so every local, cloud, and future session inherits them automatically.

---

<a id="section-5"></a>

## 5. Scope Boundary of Plan 055

- Plan 055 establishes only this standalone bilingual standard and its long-term mandatory-reading mechanism under the existing governance approval workflow;
- It does NOT delete existing Linux code, initiate refactoring, implement a Linux product release, change the current Windows 10/11 x64 product goal, or interfere with parallel development branches;
- Final delivery ([PR 050](../roadmap/PLAN.md#pr-050)) MUST implement the platform isolation verification and automated enforcement checks specified here.
