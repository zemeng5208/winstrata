# File and registry collection

[简体中文](../zh-CN/architecture/FILE-REGISTRY.md) · [Session](../../kernel/src/file_registry.rs) · [Publisher](../../core/src/file_registry/mod.rs) · [Plan 009](../roadmap/PLAN.md#pr-009)

Plan 009 observes one chosen directory and one `HKEY_CURRENT_USER` subkey. The watches run in the ordinary user process through Windows APIs already linked by `windows-sys` 0.61.2. `CatalogHost` still returns `image_absent` and does not load `strata-fs.sys` or `strata-reg.sys`. Process and network collection remain PR 010 and PR 011. The service IPC from PR 008 is unchanged and is not the path that starts these watches.

## On-demand lifetime

`FileRegistrySession` counts subscriptions for `files` and `registry` only. The limit is the same 64 used by the driver manager. The first subscription starts that channel. Another subscription does not start it again. The last release stops the channel and joins its thread. A failed stop leaves the channel resident, and the next subscription does not start a second thread. A fault in one channel leaves the other channel ready. Dropping the session stops a channel that is ready or whose stop already failed.

`FileRegistry::open` rejects a directory that is not absolute or that contains a control character, and a subkey that is empty, longer than 200 characters, has more than eight components, or contains an empty component, `.`, `..`, `/`, `:`, or a control character. The error is `collector_fault` with the fixed detail `invalid collection scope`. The rejected path is not copied into the error. Plan 017 can attach a `CollectionScope`. While it is set, a path or registry key outside that scope is not published. The publisher records a filtered gap and does not copy the rejected path into it. With no scope, plan 009 behavior is unchanged. Linux still returns `unsupported` from subscribe.

On Linux, start returns `unsupported` and publishes nothing. The other channel stays idle.

## Windows sources

The file thread calls `ReadDirectoryChangesW` on the directory handle. The watch is not recursive. The buffer is 4096 `u32` values, 16 KiB. The filter is file name, directory name, attributes, size, last write, and security. Last access is not requested. The provider is `windows.read_directory_changes`.

The registry thread opens one subkey with `RegOpenKeyExW` and arms `RegNotifyChangeKeyValue` for name and last-set changes. The watch is not recursive. After each signal the thread re-arms before it reads the next snapshot, then diffs the snapshots. A snapshot is incomplete when it passes 64 values, a name or value needs more than 256 UTF-16 characters or 1024 bytes, a name is not UTF-16, or a name repeats. An incomplete snapshot publishes one `registry.key_changed` with partial reason `snapshot_incomplete` and does not invent a value diff. An empty diff publishes nothing. The provider is `windows.reg_notify`.

The startup signal is sent only after the first read or notify is pending. The caller waits up to 2 seconds. If the registry notify is already signaled between the baseline snapshot and the first arm, the thread publishes one `lost` gap and takes a new snapshot. Those values cannot be attributed one by one. A later `lost` gap does not stop the watch. A hard read, arm, or snapshot failure publishes `source_failure`, records the health error, and exits the thread. `FileRegistry::view` and `subscribe` move a ready channel to failed when that health is set. `release` does not, so a failed thread can still be stopped.

Win32 `ERROR_NOTIFY_ENUM_DIR` (1022) and a successful read of zero bytes are `lost` gaps with no range. `ERROR_OPERATION_ABORTED` (995) is cancellation. The unit test covers that classification. This plan does not force a live overflow of the 16 KiB buffer.

## Protocol fields

Events use protocol v1 on the existing event bus. The device is the id passed to `open`. The epoch is `fs-N` or `reg-N` and increases on each successful start. The sequence starts at 1 for that epoch and is shared by observations and gaps. `precision_ns` is omitted. These APIs do not report the precision of the change time, and the timestamp is the user-mode clock when the buffer is observed. A clock outside the `i64` nanosecond range saturates. It is not replaced with zero. The process field is omitted. No acting process id is available here. File bytes are not read.

A file or directory name becomes `file.added`, `file.modified`, `directory.added`, or `directory.modified` when metadata says so. The subject is the full path when that path fits an identifier. Otherwise the subject is the relative name and the partial reason is `absolute_path_exceeds_identifier`. If neither fits, there is no subject and the reason is `path_exceeds_identifier`. The user is the owner SID from `GetNamedSecurityInfoW`. When the SDDL also fits an identifier, it is the attribution evidence and completeness is complete. When the SDDL does not fit, the evidence stays the provider name and the partial reason is `permissions_exceed_identifier`. A missing security descriptor uses `permissions_unavailable`. A removal is `path.removed` with `security_unavailable`. The owner is not invented. A rename is a pair, `path.renamed_from` then `file.renamed_to` or `directory.renamed_to`. An unpaired rename uses `rename_unpaired`. A name that is not Unicode is `path.unreadable` with `name_not_unicode_{action}`. A name that escapes the root, including `..` or an absolute name, is not opened and uses `path_escapes_root`. A reparse point uses `reparse_unresolved` and is not claimed as a file or directory. A missing metadata result uses `metadata_unavailable`.

A registry subject is `HKCU\{subkey}\{name}`. The user is the current process identity when that identity fits an identifier. The evidence is `t={type};a={hex}` for an addition, `t={type};b={hex}` for a removal, and `t={type};b={before};a={after}` when the type is unchanged. A type change uses `tb`, `ta`, `b`, and `a`. Hex is lowercase. The whole evidence string must fit an identifier. It is not truncated. When it does not fit, the evidence is the provider name and the partial reason is `value_exceeds_identifier`. The baseline present at subscribe is not published.

## Limits that remain

No signed driver was loaded or unloaded. Secure Boot and HVCI were not exercised. Windows 10 and Windows 11 clients were not tested by this change. The Windows tests are compiled for `x86_64-pc-windows-msvc` and run in the Windows workspace job. That job is Windows Server 2022. E2E-02 still requires the GUI query over a chosen disk, other users, and the broader file and registry scenario. Before-and-after plugin attribution remains PR 019. The file plugin remains PR 021.
