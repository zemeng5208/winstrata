# Per-user Core configuration and storage

[简体中文](../zh-CN/architecture/CORE-STORAGE.md) · [Rust library](../../core/src/lib.rs) · [Plan 006](../roadmap/PLAN.md#pr-006)

PR 006 provides the local Core session, its settings and the storage needed to reopen them. The caller gets the identity of the running OS account. Windows uses the process token's user SID and Linux uses the effective UID.

## Opening a session

Call `Core::open()` as the current user. On Windows the user's Local AppData directory contains `WinStrata/location.json` and the session lock. The locator remembers the storage choice. The OS holds an exclusive lock on `session.lock` for the life of the session. The default data root is `D:\WinStrata\`. On Linux the locator lives under the user's XDG state directory, and the first session requires an explicit data directory.

The managed file is `<selected-root>/<user-id>/core.json`. It contains format version 1, the owner, a workspace identifier and the settings. Keep the user directory private. Core checks ownership and permissions and rejects redirected paths. Use `storage_state()` to distinguish `Ready` from `NeedsDirectory { kind, detail }`. An operation that detects a missing, full or unwritable volume suspends persistence and stops live mode. Opening returns an error if the locator is unreadable or invalid, or if another session holds the lock.

## Changing configuration and location

- Start with live monitoring stopped and an empty monitoring scope. Use `update_settings` to record explicit choices in the `monitored_roots` and `start_at_login` fields of `Settings`. Monitored roots use absolute paths without parent traversal. The start-at-login preference defaults to false. OS startup registration belongs to its integration.
- Use `start_live` after selecting at least one monitoring root. Inspect `run_state()` for `Stopped`, `Live` or `History`. Use `stop` to stop the session or `enter_history` to leave live mode and enter history. Starting a new session leaves monitoring stopped.
- Stop live monitoring and use `select_directory` for an explicit storage choice. For a move, Core copies the current user’s regular files and directories, rereads both trees, and compares names, types and bytes before atomically publishing the new locator. It rejects overlapping trees, links and conflicting destination contents. The source remains in place.
- If the existing workspace is unavailable, restore it and reselect its root. Core checks that its marker still matches the locator. To start fresh elsewhere, explicitly call `select_empty_directory` with an independent, empty per-user directory. Earlier data stays on the old volume. For an accessible workspace, use the verified migration path.

## Verification scope

Use `cargo test --locked -p winstrata-core` on Linux and Windows. Check startup, individual settings, restart, storage loss and recovery, private filesystem access and migration of the Core configuration. The workspace CI runs the tests on Linux and Windows Server 2022. Windows 10/11 client acceptance, database and event-format migrations, encrypted backup/restore and end-to-end monitoring use the later roadmap items. See the [engineering record](../records/STATUS.md) and [data architecture](DATA.md#section-8).
