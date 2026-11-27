//! 按宿主操作系统选择计数器来源。采样只报告这次调用真正读到的值。

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod host;

#[cfg(windows)]
#[path = "windows.rs"]
mod host;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("StrataPulse supports Windows and the Linux development host");

pub use host::Sampler;
