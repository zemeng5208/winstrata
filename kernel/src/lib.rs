#![cfg_attr(not(test), forbid(unsafe_code))]

//! 驱动目录与按需生命周期。
//!
//! 本计划没有已签名的驱动映像，也没有文件、注册表、进程或网络采集。
//! `CatalogHost` 拒绝加载，不调用服务控制管理器，也不改变签名、Secure Boot 或 HVCI。

mod manager;

pub use manager::{
    CatalogHost, Driver, DriverHost, DriverView, HostError, MAX_SUBSCRIPTIONS, Manager, Phase,
};

#[cfg(all(test, windows))]
mod scm;
