# Privileged service, controlled IPC and driver management

[简体中文](../zh-CN/architecture/SERVICE-KERNEL.md) · [Service](../../services/src/lib.rs) · [Driver manager](../../kernel/src/lib.rs) · [Plan 008](../roadmap/PLAN.md#pr-008)

Plan 008 adds `winstrata-service` and `winstrata-kernel`. The service exchanges versioned, length-prefixed JSON with the current user. The kernel crate tracks four driver subscriptions. The product host does not load a kernel image. Plan 009 adds user-mode file and registry watches in the same kernel crate. Process and network collection remain later plans. Those watches are specified in [File and registry collection](FILE-REGISTRY.md).

## Privilege

`Service::listen_catalog` reads identity and privilege from the operating system. On Linux the identity is `uid:` followed by the effective user id, and the privilege is `NotWindows`. On Windows the identity is the process token SID. The privilege is `WindowsElevated` only when `TokenElevation.TokenIsElevated` is nonzero.

Subscribe and release require `WindowsElevated`. Status and audit do not. A Linux process and an ordinary Windows token receive `privilege_unavailable` before the driver host is called. `GuiClient` exposes only `status`. The server also rejects roles `gui`, `plugin` and `script` for subscribe, release and audit with `permission_denied`.

The operating system authenticates the user, not the process role. A process running as the service owner can claim role `core`. That limit is AS-005. Plugin capability separation remains PR 014 and PR 015.

## Transport

A frame is a little-endian `u32` length and then UTF-8 JSON, at most 16 KiB. A read or write waits at most 2 seconds. The accept loop serves one connection at a time. On Windows the pipe instance is created before `listen` returns. The 200 ms wait checks the stop flag and leaves that instance open. A client retries for up to 2 seconds while the next instance is not yet listening. A closed-pipe read, including Win32 error 232, is end of stream.

Linux uses a Unix-domain socket. A unique endpoint is a mode `0700` directory containing a mode `0600` socket; shutdown removes that directory. The default endpoint is `$XDG_RUNTIME_DIR/winstrata/service.sock` after checking that the runtime directory is private. Startup unlinks a stale socket name. Shutdown does not remove the default socket, so a replacement listener is left alone. `SO_PEERCRED` supplies the peer user, and that uid must equal the service effective uid. If the credential cannot be read or the uid differs, the connection is dropped and `faults` increases. The bytes are not parsed and no audit row is written.

Windows uses an overlapped named pipe. A unique name is `\\.\pipe\winstrata-<pid>-<n>`. The default name is `\\.\pipe\WinStrata-<sid>`. The DACL is `D:P(A;;GA;;;<sid>)`, and the pipe sets `PIPE_REJECT_REMOTE_CLIENTS` with one instance. The client opens the pipe with `SECURITY_IMPERSONATION`, so the service is allowed to impersonate it. After a connection arrives, the service impersonates the client, reads the thread-token SID, and reverts before it calls the driver manager. A connection that cannot be impersonated is closed without a reply. A token that fails the DACL is rejected by `CreateFile`. The service does not observe that call and does not audit it. The pipe ACL test and the Service Control Manager tests compile only for Windows.

Requests use protocol version 1. Unknown fields, duplicate keys, a driver other than `strata-fs`, `strata-reg`, `strata-proc` or `strata-net`, and a driver field on the wrong operation return `invalid_record`. Malformed JSON returns `invalid_json`. Another major version returns `unsupported_version`. The claimed user must equal the peer and the peer must equal the service owner; otherwise the code is `identity_mismatch`.

The audit log keeps 64 records. The next record drops the oldest and increases `audit_dropped`. `generation`, `audit_dropped` and `faults` travel as decimal strings. A host panic becomes `driver_fault`, increments `faults`, and the next request still runs.

## Driver subscriptions

`Manager` tracks `strata-fs`, `strata-reg`, `strata-proc` and `strata-net` separately. Each module accepts 64 subscriptions. The first successful load enters `ready` and increases `generation` by one. Later subscriptions do not load again. The last release calls unload before the count changes, so a host panic keeps the old count. A failed unload sets the count to zero and the phase to `unload_failed`. The next subscription does not load again; it returns the module to `ready` because the image may still be resident. `fault` clears the count. If a resident module cannot be unloaded, the phase remains `unload_failed`. One module's fault does not change the others.

`CatalogHost` is the host used by `listen_catalog`. Its load and unload both return `image_absent`. They do not call the Service Control Manager and they do not change driver signing, Secure Boot or HVCI. A unit test that constructs `Session` with `Privilege::WindowsElevated` checks the decision only. It is not an elevated Windows process. A `ready` phase in those tests means the test host returned success.

On a Windows host, the Service Control Manager tests enumerate existing driver services, create `WinStrata008MissingImage` for the absent file `C:\Windows\Temp\WinStrata008-missing.sys`, require `StartService` to fail with Win32 error 2 or 3, and delete that service. If the name already exists, deletion happens only after `QueryServiceConfig` reports that same image. A restricted token must fail to open the manager with create access after an unrestricted open has succeeded. A second Windows test requires the same kind of restricted token to fail opening the service pipe with access denied, while an ordinary client can still query status. Those tests do not start a file that exists and do not disable signature checks.

The `winstrata-service` binary listens on the default endpoint, waits for one byte from standard input, and shuts down. It is not installed as a Windows service.

## Verification boundary

Linux ran the manager and service tests. Windows Server 2022, on the corrected head, passed the manager tests, the three Service Control Manager tests, the live pipe exchange, and the restricted-token denial for both SCM create access and the pipe. No signed INF/CAT package was built or loaded. Collection remains PR 009 through PR 011. E2E-14 and E2E-15 remain later scenarios. Commands, counts and remaining gaps are in the [engineering record](../records/STATUS.md).
