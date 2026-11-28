# Desktop error register

[简体中文](../../zh-CN/records/modules/desktop.md) · [Global register](../ERRORS.md) · [Open items](../OPEN_ITEMS.md)

Authoritative error records for `apps/desktop`. [AG-63](../../../AGENTS.md#ag-63) requires these fields. STATUS.md is not a substitute.

<a id="err-027"></a>

## ERR-027

- Error ID: ERR-027
- Discovery time: 2026-11-28, during Windows 11 local verification of `apps/desktop` after main `4f257ae`
- Related PR: plan 054, branch `pr/054-windows11-remediation` (source behavior from plans 016 and 030)
- Environment: Windows 11 Home Chinese x64 (10.0.26200, build 26200, ordinary unelevated user), Microsoft Edge WebView2 Runtime 158.0.4370.45 and Visual Studio Build Tools x64 `WebView2Loader.dll` 1.0.2903.40
- Reproduction method: inspect and run `apps/desktop/src/host.rs` (`run_window`) and `cargo test --locked -p winstrata-desktop`
- Evidence: (1) `find_loader` checked only the executable directory and `%ProgramFiles(x86)%\Microsoft\EdgeWebView\Application` (where EdgeWebView runtime binaries do not expose a standalone `WebView2Loader.dll`), missing VS/BuildTools-shipped x64 `WebView2Loader.dll` via `vswhere.exe`; (2) `invoke_controller` failed to call `AddRef` on `ICoreWebView2Controller` and discarded the pointer on return; (3) `WM_SIZE` was unhandled and `WM_PAINT` delegated directly to `DefWindowProcW` without showing the fallback page address; (4) `WM_DESTROY` leaked `WindowData` instead of closing/releasing the controller and reclaiming the box; (5) `hide_to_tray` hardcoded `szTip` to `"WinStrata"` instead of reflecting pending storage alerts ([OI-057](../OPEN_ITEMS.md#oi-057)).
- Root cause: incomplete COM reference management, missing Visual Studio/SDK `WebView2Loader.dll` discovery, missing resize/paint/destroy handlers, and unimplemented storage-status tray tooltip formatting in `apps/desktop/src/host.rs` and `prefs.rs`.
- Impact: embedded WebView2 attachment, client-area resizing, fallback URL rendering, resource teardown, and pending-storage tray notifications failed or remained unimplemented on Windows 11.
- Fix plan: implemented `format_tray_tip` in `apps/desktop/src/prefs.rs` and re-exported it in `lib.rs`; updated `apps/desktop/src/host.rs` to verify x64 PE machine `0x8664` and discover `WebView2Loader.dll` via `vswhere.exe` (`CREATE_NO_WINDOW`), `AddRef`/store/close/`Release` `ICoreWebView2Controller`, handle `WM_SIZE`, draw `origin` in `WM_PAINT` when unattached, reclaim `WindowData` in `WM_DESTROY`, and query `storage-status` to set pending-category tray tooltips before hiding. Added tests in `apps/desktop/tests/official_gui.rs`.
- Retest result: `cargo test --locked -p winstrata-desktop` passed all 11 integration tests with `RUSTFLAGS="-Dwarnings"`, including `tray_tip_shows_pending_storage_categories_and_keeps_files` and `windows_official_window_attaches_webview2_resizes_hides_to_tray_and_quits`.
- Current state: resolved on branch `pr/054-windows11-remediation`
