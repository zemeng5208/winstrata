# Trace plugin

[简体中文](../zh-CN/architecture/TRACE.md) · [Plugin host](PLUGINS.md) · [Plan 009 watch](FILE-REGISTRY.md) · [Plan 021](../roadmap/PLAN.md#pr-021)

Plan 021 is `strata.trace` (StrataTrace), the file plugin. It watches directories from inside `PluginHost` and publishes user-mode file metadata on the existing event bus. The host pattern matches `strata.registry`. It has no package and does not start `winstrata-runtime`. It does not load `strata-fs.sys`. It does not replace the plan 009 one-directory watch. Environment variables and PATH remain plan 020. Content hashes and signatures remain plan 033. Plan 019 user-mode registry evidence is not kernel attribution. This file evidence is user-mode metadata only.

## In-process host

`load`, `update`, `pin`, `grant`, `call`, and `ping` return `builtin_closed`. `uninstall` returns `builtin_retained`. The version is 0.1.0. The grant is `event.publish`. Batch start is allowed.

`host.json` stores `trace_enabled`, `trace_availability`, and `trace_scope`. An empty scope does not name a product default disk. On Linux, start returns `unsupported`, publishes nothing, and the report stays enabled and `unavailable`. `enable_batch` does not retry a saved `unsupported` result. `disable` returns availability to `ready`.

The existing plugins page lists this availability through the host report. This plan adds no page and no GUI source row. It does not add Fluent UI, Astryx, or a new visual system. The official shell already lists plugin availability from the host report.

## Scope

`set_trace_scope` takes 1 to 4 existing directories. It rejects relative paths, `.` and `..`, control characters, symlinks, non-directories, duplicates, and other users' private profile directories. The rejected path is not copied into the error. The detail is the fixed string `invalid trace scope`. The scope cannot change while the watch is running.

## Windows watch

The Windows watch uses `ReadDirectoryChangesW`. It is not recursive. The buffer is 16 KiB. The provider is `windows.read_directory_changes`. The device is `strata.trace`. The epoch is `fs-N`. File bytes are never read. There are no content versions and no content hashes.

The metadata evidence is `sb={before-size-or-dash};sa={after-size-or-dash};pd={0-or-1}`. A dash stands where a size is absent. `pd` is the permission-changed flag and is 0 or 1. When both owners are present and differ, and the longer string still fits an identifier, the evidence appends `;ob={before-owner};oa={after-owner}`. When an SDDL fits, it appends `;sd={sddl}`. Neither field is truncated. When the SDDL does not fit, the evidence keeps the size pair and the partial reason is `permissions_exceed_identifier`. When the size pair itself does not fit, the evidence is the provider name and the partial reason is `metadata_exceeds_identifier`. A removal does not invent a current owner.

`precision_ns` is omitted. The process field is omitted. These APIs do not report the precision of the change time or the acting process. This is not kernel attribution.

## Events

Event types follow plan 009: `file.added`, `file.modified`, `directory.added`, `directory.modified`, `path.removed`, `path.renamed_from`, `file.renamed_to`, and `directory.renamed_to`. An incomplete snapshot publishes `directory.changed`. Partial reasons are `path_escapes_root`, `reparse_unresolved`, `metadata_unavailable`, `rename_unpaired`, `name_not_unicode_*`, `snapshot_incomplete`, `security_unavailable`, `permissions_unavailable`, `owner_unavailable`, `absolute_path_exceeds_identifier`, `path_exceeds_identifier`, `permissions_exceed_identifier`, and `metadata_exceeds_identifier`. The baseline present at start is not published. A lost 16 KiB overflow is a `lost` gap with no per-file attribution. Pause publishes one `paused` gap per directory per pause generation and advances the baseline without a per-file diff. The watch ring keeps the latest 128 events.

## History

`trace_query` returns that watch ring. `trace_history` returns live events whose subject is the exact path. When a plan 017 baseline entry equals that path, it also returns a projection `trace.reconstructed` with evidence `state=reconstructed`. It returns one `trace.path_shared` for each other source device whose subject or evidence equals that exact path. That evidence is `with={device}`. The provider of the projections is `winstrata.trace`. Path equality is not a causal claim and is not a timing correlation. Timing overlap alone does not create a link. Projections omit the process field and `precision_ns`.

## Limits that remain

No signed file minifilter was loaded. The authoring host is Linux. The Linux grammar tests passed there. On Windows Server 2022 head `dc9d9e378f9f585929f6acfc18944411f1bbcc75`, `windows_file_watch_overflow_records_a_lost_gap` and `windows_trace_records_metadata_without_content` passed. The earlier `source_failure` is [ERR-019](../records/ERRORS.md#err-019) and is resolved. No GUI session was run. The counts are in the [engineering record](../records/STATUS.md). E2E-02 and E2E-03 are not complete. OI-016 and OI-018 are closed. The file evidence is not kernel attribution. Plan 020 is on main and this plan does not re-implement it.
