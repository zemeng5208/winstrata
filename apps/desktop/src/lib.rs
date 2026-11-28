//! 官方默认桌面界面。
//!
//! 窗口和托盘属于这个宿主。引导和模式需要同一条 Core 会话，因此桥接在进程内持有它。
//! 页面不能直接订阅或释放驱动。官方界面不能卸载。第三方界面替换不在本计划的验收里。

mod bridge;
mod observe;
mod prefs;
mod session;

#[cfg(windows)]
mod host;

pub use bridge::Bridge;
pub use observe::{Observation, PluginView, observe_at, observe_service};
pub use prefs::{
    ClosePreference, Language, Preferences, Theme, format_tray_tip, preference_directory,
    uninstall_official_gui, OFFICIAL_GUI,
};

#[cfg(windows)]
pub use host::{
    close_current_window, current_window_visible, last_window_snapshot, loader_present,
    resize_current_window, restore_current_window, run_window,
};

pub fn asset_directory() -> std::path::PathBuf {
    let shipped = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("ui")))
        .filter(|dir| dir.join("index.html").is_file());
    shipped.unwrap_or_else(|| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist")
    })
}
