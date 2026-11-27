# Tag plugin

[简体中文](../zh-CN/architecture/TAG.md) · [Index](../INDEX.md) · [Plugin host](../../core/src/plugins/mod.rs) · [Plan 029](../roadmap/PLAN.md#pr-029)

Plan 029 tags a real event, file, process, software, registry, or system object, and a staged packet, by hand, by an enabled user script, or by another running plugin. The program is `winstrata-strata-tag`. It uses the existing native frame protocol: absolute `--manifest` and `--state` paths, and one JSON frame at a time. It does not add a frame operation, a permission, or a crate from outside the workspace. There is no built-in virus database.

## Registration

`PluginHost::open` registers `strata.tag` only when `winstrata-strata-tag` is a regular file beside the runtime passed to the host. The seeded entry is builtin, version 1.0.0, and stays disabled until enabled. Its capabilities are `tag.apply`, `tag.query`, and `tag.policy`. Its permissions are `event.publish`, `event.subscribe`, `storage.private`, and `history.read`. Its events are `tag.applied`, `tag.rejected`, and `tag.late`. Third-party `install` still returns `builtin_closed`. `uninstall` still returns `builtin_retained`. Without the program, `enable` still returns `implementation_absent`.

Enable starts the tag process. Pause and resume keep the process. While the plugin is paused, `tag.apply` records `paused` and does not write a packet. Disable and stop reply, write the bytes `joined` to `released`, and exit 0.

## Apply

The host is the only writer of a tag decision and of packet bytes. The plugin process stores the decision the host already made. Caller fields `actor`, `time_unix_ns`, and `safety` are discarded. The host writes `safety` as `none`. A missing tag is not a safety conclusion, and an applied tag is not one either.

The actor stamp depends on the caller. A call from `strata.tag` is `user`, and `actor_id` is the Core user. A call from `strata.mcp` would be `agent`. That caller is not installed. The gap is [OI-053](../records/OPEN_ITEMS.md#oi-053). Any other caller that reaches `tag.apply` through `plugin.call` is `plugin`, and `actor_id` is that plugin id.

`kind` is `event`, `packet`, `file`, `process`, `software`, `registry`, or `system`. `subject` is 1 to 256 bytes. `attribution` is `observed`, `inferred`, or `unknown`. `evidence` is empty or 1 to 256 bytes. A packet also needs a positive `packet` id. `reason`, `source`, and `confidence` are empty together, or each is 1 to 128 bytes with no surrounding whitespace and no control characters. Any other mix is `tag_rejected`. All three empty is `untagged` for a non-packet, and the packet ledger returns the same code when the policy allows a write.

The plugin appends the record to `tags.jsonl` and overwrites `last.json`. `tag.query` returns the last matching subject or packet. `tagged` is true only when that record's code is `applied`. `safety` in the answer is `none`. An unknown subject returns `code` `none` and `tagged` false.

## Packet policy

Packet bytes stay in the host `PacketLedger`. They are not placed in the 1,024-byte frame. `stage_packet` keeps bytes in memory and creates no file. The default policy is `memory_only` and the default cap is 256 KiB. `tag.policy` accepts `memory_only` and `tagged_only`. It rejects `all`, `persist`, and `persist_all` with `policy_rejected` before any file is created. There is no persist-all API. Shrinking the cap releases the oldest retained bytes. The id remains, and a later persist returns `unavailable`.

`persist` order is unchanged: unknown id, then memory-only, then a missing tag, then a rejected tag, then a released, empty, or truncated packet, then `create_new` plus `sync`. A failed write removes its own partial file. If the packet file is written and the plugin exchange then fails, the host deletes that file. Disabling the tag plugin cannot switch on full packet persistence, because the write runs only inside `tag.apply` while the plugin is running and not paused.

## Bus

`applied` publishes `tag.applied`. `unavailable` publishes `tag.late`. `untagged`, `memory_only`, `tag_rejected`, `unknown_packet`, and `paused` publish `tag.rejected`. `policy` publishes nothing. The device is `local-plugin-host` and the provider is `strata.tag`. The sequence is the live plugin sequence. Attribution is observed or inferred with the evidence id, otherwise unknown. The user field is set only when the actor is `user`. Completeness is complete. A missing bus skips publication and still keeps the private record.

## Script and plugin attribution

The script workbench operation `tag` does not start an interpreter. An enabled script with a per-script `plugin.call` grant returns `code` `tag_request`. The host then calls `tag.apply` with actor `script` and `actor_id` equal to that script id. Without the grant the workbench returns `permission_denied` and writes no tag line. The script plugin manifest does not gain `plugin.call`. A direct `call` of `tag.apply` on `strata.script` fails because that manifest does not provide the capability.

A third-party plugin with `plugin.call` can call `tag.apply`. The host stamps actor `plugin`.

## Official page

The official page at `/tags` composes the existing shadcn-admin [Button](../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx), [Input](../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx), and [Badge](../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx). It does not add Fluent UI, Astryx, or a new visual system. It does not send `enable` or `grant`. The bridge command `tag-apply` forwards a payload only while `strata.tag` is already running. Before a storage location is chosen it returns `enabled` false and `safety` `none`, and it does not create the plugins directory. The Windows page session was not opened. That gap is [OI-054](../records/OPEN_ITEMS.md#oi-054).

## What this plan does not do

This plan does not capture from a NIC inside the plugin, export PCAPNG, draw topology, or discover a network. Those stay plan 031. [E2E-04](../roadmap/ACCEPTANCE.md#e2e-04) stays open. Untagged connection metadata stays [OI-025](../records/OPEN_ITEMS.md#oi-025). [E2E-05](../roadmap/ACCEPTANCE.md#e2e-05) stays plan 030. This plan does not start plan 028 or plan 030. Local-agent tagging stays [OI-053](../records/OPEN_ITEMS.md#oi-053). The Windows `cargo test --locked --workspace` run for this plugin was not executed on the Linux authoring host. That gap is [OI-052](../records/OPEN_ITEMS.md#oi-052).

## Verification

The evidence is in the [engineering record](../records/STATUS.md).
