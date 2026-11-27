# Filter plugin

[简体中文](../zh-CN/architecture/FILTER.md) · [Index](../INDEX.md) · [Plugin host](PLUGINS.md) · [Data policy](DATA.md#section-8) · [Script plugin](SCRIPT.md) · [Plan 028](../roadmap/PLAN.md#pr-028)

Plan 028 is the official plugin `strata.filter` in `plugins/strata-filter`. The display name is StrataFilter. The program is `winstrata-strata-filter`. The direct prerequisites are plans 021, 025, and 027. The process speaks the existing length-prefixed JSON frames. It does not add a frame operation, a permission name, or a crate from outside the workspace.

## Registration

`PluginHost::open` registers `strata.filter` only when `winstrata-strata-filter` is a regular file beside the runtime passed to the host. The seeded entry is builtin, version 1.0.0, and stays disabled until enabled. Its capability is `filter.apply`. Its permissions are `storage.private` and `plugin.call`. It declares no events, so it cannot publish a bus event that it would then observe. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`. Without the program, `enable` still returns `implementation_absent`.

Enable starts the filter process. Pause keeps the process and holds new items out of the buffer. Disable and stop reply, write the bytes `joined` to `released`, and exit 0.

## Baseline protection

`protect` drops every item whose actor is `self`. That covers ordinary internal writes, logs, and IPC, including a later observation of this plugin's own batch file. It also drops `packet.raw` when `tagged` is absent or false. An actor `other` with kind `outside` is kept by this baseline, including a change to a WinStrata settings or database path. `PluginHost::protect_observation` calls the same function and does not start the process. The private `batches` directory is not created until enable.

A script verdict cannot put a dropped self item or an untagged raw packet back into a batch. Those items are removed before the script sees the batch.

## GUI rules and batch script

`filter.apply` accepts one JSON object of at most 1,024 bytes and no control characters. `rule` adds an include or exclude for `process`, `app`, `path`, `event`, `protocol`, `ip`, `port`, or `iface`. A path rule that ends in `/` or `\` matches a prefix. Other rules match the whole field. Exclude wins. A non-empty include list for a field requires a match. `tune` sets deduplication, aggregation of consecutive non-critical metrics, a sample interval from 1 to 32, a buffer from 1 to 32, a flush interval up to 60,000 ms, compression, and archive `keep` or `drop-metric`. `script` stores one StrataScript id. An empty id clears it.

`apply` stages items that pass the baseline and the GUI rules. Deduplication, sampling, aggregation, and `drop-metric` happen before the script. `flush`, a full buffer, or an elapsed interval asks for one script run of the whole staged batch. The host writes that batch to the script private `batch.txt` and calls `script.workbench` once. The script prints one `keep` or `drop` line per staged row. A fault, a missing script, a length mismatch, or a script plugin that is not running is logged to `fallback.log` and the basic rules are written instead. The script still cannot restore a baseline drop.

## Batch writes and I/O

Kept rows are written as one `batches/bNNNN.wfb` file per flush. The file starts with `WFB1`. Identical consecutive rows are stored in a counted form when that form is smaller than the raw lines. The result reports `seen`, `kept`, `suppressed`, `excluded`, `sampled`, `deduped`, `aggregated`, `archived`, `scripted`, `avoided`, `overhead`, `raw`, and `stored`. `avoided` is the events that were not kept. `overhead` is the bytes of the batch files. The same numbers are in `stats.json`.

## Official page

The official page composes the existing shadcn-admin [Button](../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx), [Input](../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx), and [Badge](../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx). It does not add Fluent UI, Astryx, or a new visual system. It does not send `enable` or `grant`. The bridge command `filter-apply` forwards a payload only when the plugin is already running. Before a storage location is selected, the bridge returns kept, avoided, and overhead as zero and does not create a plugin directory.

## Outside this plan

Plans 029, 030, and 038 are outside this plan. Tag persistence, deletion, and snapshot comparison stay there. Disk, network, and storage budgets stay [OI-051](../records/OPEN_ITEMS.md#oi-051). The Windows Server 2022 workspace job on `bc009ea2ee3481ae8a9fea295c246380249cd32a` passed the filter tests. That closes [OI-052](../records/OPEN_ITEMS.md#oi-052) for that head. The official filter page was not opened on Windows. That is [OI-053](../records/OPEN_ITEMS.md#oi-053). [E2E-04](../roadmap/ACCEPTANCE.md#e2e-04), [E2E-05](../roadmap/ACCEPTANCE.md#e2e-05), [E2E-07](../roadmap/ACCEPTANCE.md#e2e-07), and [E2E-08](../roadmap/ACCEPTANCE.md#e2e-08) stay open.
