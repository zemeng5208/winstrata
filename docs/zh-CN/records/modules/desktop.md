# Desktop 错误登记

[English](../../../records/modules/desktop.md) · [全局登记](../ERRORS.md) · [未验收事项](../OPEN_ITEMS.md)

`apps/desktop` 的权威错误记录。[AG-63](../../AGENTS.md#ag-63) 要求这些字段。STATUS.md 不能代替本文件。

<a id="err-027"></a>

## ERR-027

- Error ID: ERR-027
- 发现时间: 2026-11-28，在 main `4f257ae` 之后对 `apps/desktop` 的 Windows 11 本机实测期间
- 关联 PR: 计划 054，分支 `pr/054-windows11-remediation`（来源行为源自计划 016 和 030）
- 环境: Windows 11 家庭中文版 x64（10.0.26200，内部版本 26200，普通未提权用户），Microsoft Edge WebView2 Runtime 158.0.4370.45 与 Visual Studio Build Tools x64 `WebView2Loader.dll` 1.0.2903.40
- 复现方法: 检查并运行 `apps/desktop/src/host.rs`（`run_window`）与 `cargo test --locked -p winstrata-desktop`
- 证据: (1) `find_loader` 仅查找可执行目录和 `%ProgramFiles(x86)%\Microsoft\EdgeWebView\Application`（该运行时目录不提供独立 `WebView2Loader.dll`），未能通过 `vswhere.exe` 查找 VS/BuildTools 自带的 x64 `WebView2Loader.dll`；(2) `invoke_controller` 未对 `ICoreWebView2Controller` 调用 `AddRef`，并在回调返回时丢弃指针；(3) `WM_SIZE` 未处理，且 `WM_PAINT` 直接交给 `DefWindowProcW`，未绘制回退页面地址；(4) `WM_DESTROY` 未关闭/释放控制器并泄漏 `WindowData`；(5) `hide_to_tray` 将 `szTip` 硬编码为 `"WinStrata"`，未展示等待中的存储提醒（[OI-057](../OPEN_ITEMS.md#oi-057)）。
- 根因: `apps/desktop/src/host.rs` 和 `prefs.rs` 存在 COM 引用计数管理不完整、缺失 VS/SDK `WebView2Loader.dll` 发现路径、缺失窗口大小/绘制/销毁处理，以及未接入存储状态托盘提示格式化等问题。
- 影响: 在 Windows 11 上，嵌入式 WebView2 挂载、客户区大小重排、回退地址渲染、窗口资源释放和存储待确认托盘提醒均不可用或行为不完整。
- 修复计划: 在 `apps/desktop/src/prefs.rs` 实现 `format_tray_tip` 并在 `lib.rs` 导出；在 `apps/desktop/src/host.rs` 校验 x64 PE 机器码 `0x8664` 并通过 `vswhere.exe`（`CREATE_NO_WINDOW`）发现 `WebView2Loader.dll`，对 `ICoreWebView2Controller` 执行 `AddRef`/保存/`Close`/`Release`，处理 `WM_SIZE`，在未挂载时于 `WM_PAINT` 绘制 `origin`，在 `WM_DESTROY` 回收 `WindowData`，并在隐藏到托盘前请求 `storage-status` 更新托盘提示。已在 `apps/desktop/tests/official_gui.rs` 新增测试。
- 重测结果: 在 `RUSTFLAGS="-Dwarnings"` 下，`cargo test --locked -p winstrata-desktop` 全部 11 项集成测试通过，包括 `tray_tip_shows_pending_storage_categories_and_keeps_files` 与 `windows_official_window_attaches_webview2_resizes_hides_to_tray_and_quits`。
- 当前状态: 已在分支 `pr/054-windows11-remediation` 解决
