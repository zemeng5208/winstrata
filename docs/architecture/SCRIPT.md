# Script plugin

[简体中文](../zh-CN/architecture/SCRIPT.md) · [Index](../INDEX.md) · [Plugin host](PLUGINS.md) · [Language runtimes](RUNTIMES.md) · [Errors](../records/modules/strata-script.md) · [Plan 027](../roadmap/PLAN.md#pr-027)

Plan 027 is the official plugin `strata.script` in `plugins/strata-script`. The display name is StrataScript. The program is `winstrata-strata-script`. The direct prerequisites are plans 015 and 016. The process speaks the existing length-prefixed JSON frames. It does not add a frame operation.

## Registration

`PluginHost::open` registers `strata.script` only when `winstrata-strata-script` is a regular file beside the runtime passed to the host. The seeded entry is builtin, version 1.0.0, and stays disabled until enabled. Its capability is `script.workbench`. Its permissions are `event.subscribe`, `event.publish`, `storage.private`, and `history.read`. Its events are `script.started`, `script.finished`, and `script.faulted`. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`.

`invoke` of `script.workbench` carries a compact JSON payload of at most 1,024 bytes with no control characters. Script source travels as base64, in one part or in numbered parts, because a frame text field cannot hold raw newlines.

## Presets

The package directory contains only `manifest.json`. No preset script is shipped in the crate, the package, or the SDK. On start the plugin creates an empty `presets` directory and writes `preset.count` as the single character `0`. If that directory ever contains a file, workbench commands report `preset_present` and do not run scripts.

## Workbench

The workbench stores user scripts in the plugin private state. The languages are `python`, `javascript`, and `typescript`. A new script is disabled. The editor returns token kinds, keyword and identifier completion, and diagnostics for an empty script, an unclosed string, and unmatched brackets. Versions increment on edit. `hot-reload` replaces the source of an enabled script, and the next run uses that source. `export` and `import` move one script. An imported script stays disabled and is not a preset.

## Execution

Run, debug, and triggers execute the real interpreters the host passes at spawn, after `bind_languages`: `--python`, `--python-prefix`, `--node`, and `--node-prefix`. The host does not search `PATH` inside the plugin, because the plugin environment is cleared on Unix. If those arguments are absent, execution returns `toolchain_missing`. A declared module token is imported before the user script. A missing module returns `dependency_missing`, and the user script does not start. Each run has a 1.2 second limit. The child is then killed, and the result is `timeout`. Logs are JSON lines in the private state. Disable stops the scheduler, kills the child, and writes `released` containing `joined`.

## Permissions

The workbench uses the five existing permission names: `event.publish`, `event.subscribe`, `history.read`, `plugin.call`, and `storage.private`. `declare` puts a name in pending. `grant` accepts only a pending permission. `revoke` returns it to pending. `event.publish` is required before a run may record an emit. `event.subscribe` is required before an event trigger fires. `storage.private` is required before the child is given write access to its private directory.

## Triggers

A manual trigger is `run`. An event trigger is `ingest` with a matching event token. A schedule trigger uses `every_ms` of at least 200, on a background thread inside this process only. A condition trigger uses `field` `count` and `cmp` `gte` or `eq`. Plugin pause, and script pause or disable, skip triggers. Collection by `strata.process` and `strata.registry` stays unchanged.

## Debugging

Python uses `sys.settrace` on the real source. JavaScript and TypeScript run under Node with a line hook in a generated harness file under the private `debug` directory. That harness is not a preset. TypeScript uses Node `--experimental-strip-types`. `debug-start`, `debug-step`, `debug-continue`, and `debug-stop` use a pause file and a control file.

<a id="containment"></a>

## Containment

When `libwinstrata_shim.so` is beside the runtime, each user-script child gets the same Landlock plus `LD_PRELOAD` shim as [plan 015](RUNTIMES.md). Read access covers the interpreter prefix, the loader paths, and the plugin state. Write access covers `debug` during debugging, and `private` only when `storage.private` is granted. If the shim or Landlock cannot be installed, the child is not started and the result is `isolation_unavailable`. This plan does not place Windows children in an AppContainer. That gap is [OI-045](../records/OPEN_ITEMS.md#oi-045), not a success.

## Official page

The official page composes the existing shadcn-admin [Button](../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx), [Input](../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx), and [Badge](../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx), plus a textarea. It does not add Fluent UI, Astryx, Monaco, or a new visual system. It does not send `enable` or `grant` for the plugin. The bridge command `script-workbench` forwards a payload only when the plugin is already running.

## Outside this plan

Plans 026, 028, 038, 043, and 045 are outside this plan. Plan 028 calls this workbench once per filter batch and falls back when that run fails. Snapshot comparison, the shared market, and MCP remain outside this plan. [E2E-07](../roadmap/ACCEPTANCE.md#e2e-07) and [E2E-12](../roadmap/ACCEPTANCE.md#e2e-12) stay open.
