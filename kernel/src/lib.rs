#![cfg_attr(not(test), deny(unsafe_code))]

//! 驱动目录、按需生命周期，以及用户态网络接口与连接观测。
//!
//! 没有已签名的驱动映像。`CatalogHost` 拒绝加载，不调用服务控制管理器，
//! 也不改变签名、Secure Boot 或 HVCI。计划 011 的网络观测不经过驱动加载。

mod manager;
mod net;

pub use manager::{
    CatalogHost, Driver, DriverHost, DriverView, HostError, MAX_SUBSCRIPTIONS, Manager, Phase,
};
pub use net::{
    Basis, CapturedPacket, Direction, Endpoint, Gap, Interface, InterfaceKind, MAX_CAPTURE_PACKETS,
    MAX_CAPTURE_WAIT, MAX_ENDPOINTS, MAX_FRAME_BYTES, Observer, PacketBody, Snapshot, Transport,
};

#[cfg(all(test, windows))]
mod scm;
