#![cfg_attr(not(test), deny(unsafe_code))]

//! 驱动目录、文件与注册表采集，以及用户态网络接口与连接观测。
//!
//! `CatalogHost` 仍然拒绝加载内核映像，不调用服务控制管理器，也不改变签名、
//! Secure Boot 或 HVCI。文件和注册表采集由 `FileRegistrySession` 按订阅启动。
//! 计划 011 的网络观测不经过驱动加载。进程采集不属于本 crate 的这一层。

mod file_registry;
mod manager;
mod net;

pub use file_registry::{Channel, ChannelView, CollectError, CollectHost, FileRegistrySession};
pub use manager::{
    CatalogHost, Driver, DriverHost, DriverView, HostError, MAX_SUBSCRIPTIONS, Manager, Phase,
};
pub use net::{
    Basis, CapturedPacket, Direction, Endpoint, Gap, Interface, InterfaceKind, MAX_CAPTURE_PACKETS,
    MAX_CAPTURE_WAIT, MAX_ENDPOINTS, MAX_FRAME_BYTES, Observer, PacketBody, Snapshot, Transport,
};

#[cfg(all(test, windows))]
mod scm;
