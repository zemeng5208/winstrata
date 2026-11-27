# Native plugin contracts and lifecycle

[简体中文](../zh-CN/architecture/PLUGINS.md) · [Index](../INDEX.md) · [Rust host](../../core/src/plugins/mod.rs) · [Runtime](../../runtimes/src/main.rs) · [Plan 014](../roadmap/PLAN.md#pr-014)

Plan 014 adds the native package contract, dependency checks, and process lifecycle. The 25 official identities stay in the catalog. Their collectors, the GUI, user opt-in, language SDKs, OS containment, the market, and signed rollback stay on later plans.

## Contract

The versioned records live in `protocols`. A manifest is a named JSON object at protocol version 1.0. It carries the plugin id, display name, `plugin_version` (`major`, `minor`, `patch`), `kind`, `runtime`, capabilities, permissions, dependencies, events, and `rescue`.

A plugin id is 1 to 64 bytes. It starts with a lowercase letter and then uses lowercase letters, digits, `.` or `-`. It cannot contain `..` or end with `.`. Capability, event, and dependency ids use the same token. `exact` requires an equal version. `compatible` keeps major 0 on the same minor with a patch at least as high as required, and keeps major 1 or higher on the same major with a minor and patch at least as high as required. An update must be strictly newer.

Permissions are `event.publish`, `event.subscribe`, `history.read`, `plugin.call`, and `storage.private`. `rescue` is legal only for `strata.rescue`. Any other manifest that sets it is `rescue_isolated`. `kind` must be `third_party` for a package this host installs. `runtime` must be `native`.

The host and the plugin process exchange one frame at a time on stdio: a little-endian `u32` length and then JSON, from 1 to 65,536 bytes. The operations are `ping`, `pause`, `resume`, `stop`, `disable`, `emit`, `invoke`, `read_state`, `fault`, and `result`. Text fields are at most 1,024 bytes and cannot contain control characters. The runtime reports its process id. The host compares that id with `Child::id()`.

## Host

`Core::open()` still uses the operating-system locator. `Core::open_at` is public for a caller that names the locator directory. `Core::open_plugins` requires a selected user directory and opens `<user>/plugins`. The control files are regular private files beside `core.json`. They are not inside `vault.wsv`. Plan 006 migration copies regular files in the user directory. This plan did not add a plugin migration test and does not encrypt these files.

| Path | Contents |
|---|---|
| `host.json` | Installed entries, grants, pending permissions, pins, and enabled state |
| `history/<id>.jsonl` | Lifecycle actions, kept to the last 1,024 lines |
| `packages/<id>/` | The running package |
| `recovery/<id>/` | The last package that was installed or successfully updated |
| `state/<id>/` | That plugin's private directory |
| `staging/<id>/` | A candidate package during update |

A package is a flat directory of regular files. Names are 1 to 48 bytes, start with a lowercase letter, and use lowercase letters, digits, `.`, `_`, or `-`. The host accepts at most 16 files and 64 KiB each, and rejects a symlink. At most 256 plugins can be installed and 64 can run.

The native program is `winstrata-runtime`. The host starts it with absolute `--manifest` and `--state` paths and sets the working directory to the private state directory. Stderr is discarded. On Unix the child environment is cleared. On Windows the child keeps the process environment, because plan 004 found that a cleared environment can stop process creation. One `ws-plugin` thread forwards frames for that child. It does not take a Core scheduler worker. Closing the host kills live children and does not mark them faulted, so the next open shows an enabled plugin as `stopped`.

## Official catalog

`list` always returns these 25 identities. They have no package in this plan. `enable` returns `implementation_absent`. `uninstall` returns `builtin_retained`. A third-party package cannot take one of these ids or set `kind` to `builtin`. Only `strata.rescue` has batch enablement turned off.

| Id | Name | Plan |
|---|---|---|
| `strata.pulse` | StrataPulse | 24 |
| `strata.net` | StrataNet | 31 |
| `strata.trace` | StrataTrace | 21 |
| `strata.apps` | StrataApps | 35 |
| `strata.registry` | StrataRegistry | 19 |
| `strata.env` | StrataEnv | 20 |
| `strata.process` | StrataProcess | 18 |
| `strata.services` | StrataServices | 22 |
| `strata.startup` | StrataStartup | 23 |
| `strata.security` | StrataSecurity | 37 |
| `strata.snapshot` | StrataSnapshot | 38 |
| `strata.rules` | StrataRules | 39 |
| `strata.health` | StrataHealth | 25 |
| `strata.timeline` | StrataTimeline | 41 |
| `strata.budget` | StrataBudget | 26 |
| `strata.mcp` | StrataMCP | 45 |
| `strata.rescue` | StrataRescue | 47 |
| `strata.filter` | StrataFilter | 28 |
| `strata.disk` | StrataDisk | 32 |
| `strata.script` | StrataScript | 27 |
| `strata.dependency` | StrataDependency | 34 |
| `strata.trust` | StrataTrust | 33 |
| `strata.update` | StrataUpdate | 36 |
| `strata.tag` | StrataTag | 29 |
| `strata.storage` | StrataStorage | 30 |

## Lifecycle

`install` copies the package into `packages` and `recovery`, creates the private state directory, and leaves the plugin disabled. The declared permissions are granted. `load` checks that every dependency is installed and version-compatible. A disabled dependency can still be loaded. `enable` records the plugin as enabled and then starts it. A failed start can leave it enabled and `unavailable` or `faulted`.

`start` refuses a disabled plugin, a dependency cycle, and a running plugin that already provides the same capability. It starts an enabled, stopped dependency. A disabled, faulted, or unavailable dependency returns `dependency_unhealthy`. The child must answer `ping` with its own process id and the manifest version. Exit code 3 is `unavailable` and does not replace a plugin that is already running. Optional `startup.json` with `ready: false`, or an invalid `startup.json`, makes the runtime exit 3 before it reads stdin.

`pause` and `resume` keep the process and drop or restore its bus subscriptions. `stop` asks for exit 0 and leaves the plugin enabled. `disable` asks for exit 0, clears enabled, and keeps history. `fault` asks for exit 7. `uninstall` applies only to a third-party plugin. The history file remains.

`update` stages the new package. When the plugin is running, a second process health-checks the staged copy before the old process stops. A decline or a failed handshake deletes the stage and leaves the old process running. After a clean stop, the host writes `packages` and `recovery` and adopts the new process. A new permission stays `pending` and is not granted. `pin` must equal the installed version. A pin blocks every other update until `unpin`.

`recover` returns the current package when its manifest is readable and the plugin is not faulted. Otherwise it restores `recovery/<id>` and starts the plugin when it is enabled. A missing or invalid recovery copy is `dependency_unrecoverable`. `enable_batch` starts plugins that are already enabled, not running, and allowed in a batch. It does not enable a disabled plugin. `strata.rescue` stays absent.

## Permissions, calls, and events

`grant` accepts only a permission that is currently pending. `revoke` removes a grant. If the installed manifest still declares that permission, it returns to pending so a later grant can restore it. Revoking `event.subscribe` drops that plugin's bus subscriptions immediately.

`call` stays in the caller's process when the caller declares the capability. Otherwise the caller needs `plugin.call`, and the host forwards the invocation to the one running provider. `read_private` requires `storage.private` and asks the plugin to read one file name in its state directory. The runtime rejects `..`, a symlink, and a name outside that directory. `emit` requires `event.publish` and a declared event type. The published event uses device `local-plugin-host`, the plugin id as provider, a new epoch from each host open, and a per-plugin sequence starting at 1. `poll` returns events already queued for that plugin's declared types. The shared bus stays up when one plugin pauses.

## What this plan does not do

Separate processes, private directories, and a failed sibling are the isolation this plan runs. They are not an AppContainer, Job Object, or seccomp sandbox. Python, TypeScript, and JavaScript runtimes and SDKs are plan 015. The official default GUI is plan 016. Onboarding opt-in is plan 017. The 25 collectors are plans 018 through 047. The market is plan 043. Signed updates and transactional rollback are plan 044. Rescue behavior is plan 047. Full GUI replacement is [AG-64](../../AGENTS.md#ag-64), recorded as plan 053. It is not implemented here and it is not plan 016's acceptance. Plan 053 adds no implementation plan ID. E2E-06, E2E-11, and E2E-15 are not complete. The evidence is in the [engineering record](../records/STATUS.md).
