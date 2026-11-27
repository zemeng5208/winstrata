//! 受控服务与当前用户 IPC。
//!
//! GUI 客户端只有状态查询。驱动订阅只接受 `core` 角色，并且只有已提升的
//! Windows 服务进程会进入驱动宿主。目录宿主没有已签名映像，因此不会加载驱动。

mod protocol;
mod session;
mod transport;

pub use protocol::{AUDIT_CAP, AuditPage, AuditRecord, DriverState, MAX_FRAME, Snapshot};
pub use session::{CoreClient, GuiClient, Service};
pub use transport::{Endpoint, Privilege, current_privilege, current_user};
pub use winstrata_kernel::{Driver, MAX_SUBSCRIPTIONS, Phase};

#[cfg(test)]
mod tests;
