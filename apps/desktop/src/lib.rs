//! 官方默认桌面界面。
//!
//! 窗口和托盘属于这个宿主。采集、服务和存储不在界面进程里长期持有。
//! 官方界面不能卸载。第三方界面替换不在本计划的验收里。

mod bridge;
mod observe;
mod prefs;

#[cfg(windows)]
mod host;

pub use bridge::Bridge;
pub use observe::{Observation, PluginView, observe_at, observe_service};
pub use prefs::{
    ClosePreference, Language, Preferences, Theme, preference_directory, uninstall_official_gui,
    OFFICIAL_GUI,
};

#[cfg(windows)]
pub use host::run_window;

pub fn asset_directory() -> std::path::PathBuf {
    let shipped = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("ui")))
        .filter(|dir| dir.join("index.html").is_file());
    shipped.unwrap_or_else(|| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui/dist")
    })
}
