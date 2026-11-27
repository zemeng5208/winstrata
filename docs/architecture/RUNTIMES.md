# Python, TypeScript, and JavaScript runtimes

[简体中文](../zh-CN/architecture/RUNTIMES.md) · [Index](../INDEX.md) · [Plugins](PLUGINS.md) · [SDK](../../sdk) · [Plan 015](../roadmap/PLAN.md#pr-015)

Plan 015 adds the three language runtimes, their SDKs, project dependency checks, and OS containment. The native contract and process lifecycle stay in [Plugins](PLUGINS.md). The editor, triggers, and presets stay on plan 027. The market stays on plan 043.

## Runtimes

A third-party manifest may set `runtime` to `native`, `python`, `javascript`, or `typescript`. Any other token is `unsupported_runtime`. The runtime cannot change in an update. A language package must contain `plugin.py`, `plugin.mjs`, or `plugin.ts`. Installing a language package before the toolchain is bound returns `toolchain_missing`.

`bind_languages` checks two project declarations. Node.js must equal `package.json` `engines.node`, currently 24.22.3. Python must be at least `toolchains/runtimes.json` `python_minimum` and stay on that major version. The caller passes absolute interpreter paths and the SDK directory. The host does not search an implicit user install and does not copy either interpreter into the repository. A binary that does not match the declaration is `toolchain_mismatch`.

On Linux, Python starts from a per-plugin `venv` created with `--without-pip` under the private state directory. On Windows that `venv` executable is only a redirector, and the job allows one process, so Python starts from the project interpreter directly. Either way the SDK runs it with `-s -B -E`, then inserts only that plugin's private module directory and, when `modules.json` lists them, the project catalog. JavaScript and TypeScript start with the bound Node.js binary. TypeScript uses Node's type stripping. Private modules and catalog modules are loaded by name from `state/node` and `plugins/catalog/node`. A module that is not there is missing. User site packages and another plugin's directory are not on the path.

`modules.json` is optional. Its `modules` names are lowercase tokens. A listed module must already exist in `plugins/catalog/python` or `plugins/catalog/node`. The host does not download it. Stop, disable, uninstall, and host shutdown remove the Python `env` directory. Private files and history stay. The next start creates the environment again.

The SDKs speak the same length-prefixed frame protocol as `winstrata-runtime`. They answer `ping` with their process id and plugin version, and they implement `pause`, `resume`, `stop`, `disable`, `fault`, `emit`, `invoke`, and `read_state`. `startup.json` with `ready` not true, or an invalid `startup.json`, exits 3 before reading stdin.

## Linux containment

Before a language process runs, the host builds a Landlock ruleset and loads `libwinstrata_shim.so` with `LD_PRELOAD`. The shim runs before the interpreter's main function. It applies the ruleset, installs a seccomp filter, and closes every inherited descriptor except standard input, output, and error. If Landlock or the filter cannot be installed, the process exits and the host returns `isolation_unavailable`. It does not continue unsandboxed.

The ruleset allows read and execute on the interpreter prefix, the SDK, the package, the dynamic linker directories, `/dev/null`, `/dev/urandom`, and `/etc/ssl`. It allows read and write on that plugin's state directory. A declared catalog is readable only for that plugin. The filter rejects socket calls, `execve`, `fork`, `vfork`, mount, and namespace changes. Threads that use `clone` still work. A file opened inside an allowed directory remains readable until the process exits; the operating system does not revoke that handle when a host permission changes. Host permissions still take effect on the next frame: revoking `event.subscribe` drops the bus subscription immediately.

The native `winstrata-runtime` process is unchanged. It is still a separate process with a private directory, not this Landlock policy.

## Windows containment

The Windows host creates an AppContainer profile with no extra capabilities and a job with one active process and kill-on-close. It grants the package SID read and execute on the interpreter prefix, SDK, package, and catalog, and read and write on the state directory. That allow entry is merged into the existing DACL. A protected private directory stays protected, and its owner allow entry stays in place. The host still rejects an extra allow entry for any SID that is not that plugin's AppContainer profile. The interpreter is created suspended inside that profile, assigned to the job, then resumed. On Windows, Python starts from the project interpreter because the venv executable is only a redirector and the job allows one process. The environment block comes from the current user token. Standard input and output are the only inherited pipes. This path was not executed on the Linux host that wrote this plan. The Windows test is `runtimes/tests/languages_windows.rs`.

## What remains later

The script editor, debugging, triggers, and empty preset rule are plan 027. The shared market is plan 043. Signed updates are plan 044. E2E-06, E2E-07, and E2E-12 are not complete. An already-open handle inside an allowed directory is not revoked mid-process. Windows 10 and 11 client runs of this containment were not made on the authoring host. The evidence is in the [engineering record](../records/STATUS.md).
