//! 驱动订阅目录，以及进程、线程、模块的用户态采样。
//!
//! 没有已签名映像时，`CatalogHost` 仍拒绝加载，不调用服务控制管理器启动驱动，
//! 也不改变签名、Secure Boot 或 HVCI。进程采样使用 procfs 或 Windows 进程与服务接口，
//! 不进入内核回调。

mod manager;
mod system;

pub use manager::{
    CatalogHost, Driver, DriverHost, DriverView, HostError, MAX_SUBSCRIPTIONS, Manager, Phase,
};
pub use system::{ModuleRow, ProcessRow, ServiceRow, SystemSnapshot, capture};

#[cfg(all(test, windows))]
mod scm;
