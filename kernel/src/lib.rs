//! 驱动目录，以及文件、注册表、进程、线程、模块和网络的用户态采集。
//!
//! 没有已签名映像时，`CatalogHost` 仍拒绝加载，不调用服务控制管理器启动驱动，
//! 也不改变签名、Secure Boot 或 HVCI。文件和注册表采集由 `FileRegistrySession` 按订阅启动。
//! 进程采样使用 procfs 或 Windows 进程与服务接口，不进入内核回调。
//! 计划 011 的网络观测不经过驱动加载。
//! `manager` 模块禁止 unsafe。进程采样和网络采样调用操作系统接口，因此本 crate 不能整体禁止 unsafe。

mod file_registry;
mod manager;
mod net;
mod system;

pub use file_registry::{Channel, ChannelView, CollectError, CollectHost, FileRegistrySession};
pub use manager::{
    CatalogHost, Driver, DriverHost, DriverView, HostError, MAX_SUBSCRIPTIONS, Manager, Phase,
};
pub use net::{
    Basis, CapturedPacket, Direction, Endpoint, Gap, Interface, InterfaceKind, MAX_CAPTURE_PACKETS,
    MAX_CAPTURE_WAIT, MAX_ENDPOINTS, MAX_FRAME_BYTES, Observer, PacketBody, Snapshot, Transport,
};
pub use system::{ModuleRow, ProcessRow, ServiceRow, SystemSnapshot, capture};

#[cfg(all(test, windows))]
mod scm;
