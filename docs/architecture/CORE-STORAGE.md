# Per-user Core configuration and hybrid storage

[简体中文](../zh-CN/architecture/CORE-STORAGE.md) · [Rust library](../../core/src/lib.rs) · [Plan 006](../roadmap/PLAN.md#pr-006) · [Plan 012](../roadmap/PLAN.md#pr-012)

PR 006 provides the local Core session, its settings and the storage needed to reopen them. The caller gets the identity of the running OS account. Windows uses the process token's user SID and Linux uses the effective UID.

## Opening a session

Call `Core::open()` as the current user. On Windows the user's Local AppData directory contains `WinStrata/location.json` and the session lock. The locator remembers the storage choice. The OS holds an exclusive lock on `session.lock` for the life of the session. The default data root is `D:\WinStrata\`. On Linux the locator lives under the user's XDG state directory, and the first session requires an explicit data directory.

The managed file is `<selected-root>/<user-id>/core.json`. It contains format version 1, the owner, a workspace identifier and the settings. Keep the user directory private. Core checks ownership and permissions and rejects redirected paths. Use `storage_state()` to distinguish `Ready` from `NeedsDirectory { kind, detail }`. An operation that detects a missing, full or unwritable volume suspends persistence and stops live mode. Opening returns an error if the locator is unreadable or invalid, or if another session holds the lock.

## Changing configuration and location

- Start with live monitoring stopped and an empty monitoring scope. Use `update_settings` to record explicit choices in the `monitored_roots` and `start_at_login` fields of `Settings`. Monitored roots use absolute paths without parent traversal. The start-at-login preference defaults to false. OS startup registration belongs to its integration.
- Use `start_live` after selecting at least one monitoring root. Inspect `run_state()` for `Stopped`, `Live` or `History`. Use `stop` to stop the session or `enter_history` to leave live mode and enter history. Starting a new session leaves monitoring stopped.
- Stop live monitoring and use `select_directory` for an explicit storage choice. For a move, Core copies the current user’s regular files and directories, rereads both trees, and compares names, types and bytes before atomically publishing the new locator. It rejects overlapping trees, links and conflicting destination contents. The source remains in place.
- If the existing workspace is unavailable, restore it and reselect its root. Core checks that its marker still matches the locator. To start fresh elsewhere, explicitly call `select_empty_directory` with an independent, empty per-user directory. Earlier data stays on the old volume. For an accessible workspace, use the verified migration path.

Settings and locator JSON are limited to 64 KiB. Unsupported versions, corrupt documents and existing broad permissions fail closed and preserve the files for recovery. Clearing the monitoring scope stops live mode. This migration verifies regular file contents, directory structure and user isolation. It rejects NTFS named data streams. Windows filesystem testing covers NTFS. ReFS has not been exercised. Capacity checks compare the caller's available bytes with the requested write size, and write failures also suspend persistence.

## Hybrid records

Plan 012 stores five record classes and one index inside the selected per-user directory. `Core::open_hybrid` opens `<user>/data/{databases,events,metrics,captures,snapshots,indexes}`. `core.json` stays beside `data`. The caller assembles one change in a `Batch` and calls `Core::commit_hybrid`. The bus does not subscribe to the store and does not write to disk. A caller that needs history puts the delivered event into a batch.

Each payload is written once, into `s{id}.wsg` in its own directory. `indexes/journal.wsj` records the commit sequence, segment high-water marks, and each record's location and CRC. It does not copy the payload. The default segment is 1 MiB. Allowed sizes are 256 bytes through 16 MiB. One record may occupy its own segment beyond that limit. One payload is limited to 1 MiB, one commit encoding to 8 MiB, and an object body to 256 KiB. Opening rebuilds the in-memory index from the journal. An incomplete journal tail is truncated. A CRC-valid commit whose payload fails its checksum or schema is rolled back together with every later commit. Queries reread the bytes and refuse damaged content. A failed write in the same process deletes a new segment and restores an existing segment to its length before that write.

- The structured directory stores objects and relations. An unchanged object body is not appended again, and an identical relation does not grow the commit.
- Events store protocol JSON. A repeated sequence from the same source is rejected before the write.
- Metrics store a second difference of timestamps and a difference of values, encoded as zigzag variable integers.
- Captures are little-endian PCAPNG. The section-header magic is `0x0A0D0D0A`, the interface block uses nanosecond resolution, and the enhanced-packet comment carries the risk tag. A packet without a tag is rejected before any write. The timestamp must be zero or positive. A changed link type starts a new segment.
- Snapshots store a path and a state identifier. They do not store file contents and do not write the filesystem from those paths. A full snapshot has no base. An incremental snapshot names its base. `materialize_snapshot` folds the chain by path. Empty paths, `..`, NUL, paths longer than 4096 bytes, repeated paths and more than 8192 entries are rejected.

`set_retention` stores a day count for one `DataClass`. `None` keeps the class indefinitely. `Some(0)` is rejected. The call does not delete data. Cleanup and quota enforcement belong to plan 030. `capacity` sums regular-file bytes in each of the six directories and counts captures separately. `reserve` uses the existing `storage::capacity`. Calling it with `u64::MAX` reports insufficient space. The test does not fill a physical disk to produce that result. A volume error suspends the Core session and stops live mode. `InvalidInput` and `InvalidData` do not suspend the session.

Plan 006 migration copies regular private files in the user directory. Hybrid files move with that directory when their mode is `0600` and they have no extra hard link.

## Verification scope

Use `cargo test --locked -p winstrata-core` on Linux and Windows. Check startup, individual settings, restart, storage loss and recovery, private filesystem access and migration of the Core configuration. The workspace CI runs the tests on Linux and Windows Server 2022. Plan 012 additionally covers the writes, queries, segments, index, capacity and corruption rollback in the previous section on Linux. Windows tests on this host, Windows 10/11 client acceptance, ReFS, encrypted backup and restore, and end-to-end monitoring remain open. See the [engineering record](../records/STATUS.md) and [data architecture](DATA.md#section-8).
