#![cfg_attr(not(test), forbid(unsafe_code))]

//! 驱动目录与文件、注册表采集的按需生命周期。
//!
//! `CatalogHost` 仍然拒绝加载内核映像，不调用服务控制管理器，也不改变签名、
//! Secure Boot 或 HVCI。文件和注册表采集由 `FileRegistrySession` 按订阅启动；
//! 进程与网络采集不属于本 crate 的这一层。

mod file_registry;
mod manager;

pub use file_registry::{Channel, ChannelView, CollectError, CollectHost, FileRegistrySession};
pub use manager::{
    CatalogHost, Driver, DriverHost, DriverView, HostError, MAX_SUBSCRIPTIONS, Manager, Phase,
};

#[cfg(all(test, windows))]
mod scm;
