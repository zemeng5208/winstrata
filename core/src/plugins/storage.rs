//! 计划 030 的存储插件登记。
//!
//! 删除确认留在 Core。这个模块只在运行时旁边找到程序时登记它。

use std::path::{Path, PathBuf};

pub(super) const PLUGIN: &str = winstrata_strata_storage::PLUGIN_ID;
pub(super) const MANIFEST: &str = winstrata_strata_storage::MANIFEST;

pub(super) fn beside(runtime: &Path) -> Option<PathBuf> {
    super::collector_beside(runtime, "winstrata-strata-storage")
}
