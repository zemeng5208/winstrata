#![forbid(unsafe_code)]

//! Core 的有界调度与事件总线入口。
//!
//! 不在存储会话打开时启动线程。调用方显式创建 `Runtime`，没有订阅或任务时不占用工作者。

use std::time::Duration;

use crate::bus::EventBus;
use crate::schedule::Scheduler;

pub const MAX_WORKERS: usize = 32;
pub const MAX_QUEUE: usize = 65_536;
pub const MAX_SUBSCRIPTIONS: usize = 1_024;
pub const MAX_ADMISSION_WAIT: Duration = Duration::from_secs(5);
pub const MAX_TASK_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    code: &'static str,
    detail: &'static str,
}

impl RuntimeError {
    pub const fn new(code: &'static str, detail: &'static str) -> Self {
        Self { code, detail }
    }

    pub const fn code(self) -> &'static str {
        self.code
    }

    pub const fn detail(self) -> &'static str {
        self.detail
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for RuntimeError {}

/// 调度和总线共用的固定上限。动态预算由后续资源插件执行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub workers: usize,
    pub task_queue: usize,
    pub event_queue: usize,
    pub global_queue: usize,
    pub batch: usize,
    pub subscriptions: usize,
    pub admission_wait: Duration,
}

impl Limits {
    pub const fn standard() -> Self {
        Self {
            workers: 2,
            task_queue: 256,
            event_queue: 1_024,
            global_queue: 1_024,
            batch: 64,
            subscriptions: 64,
            admission_wait: Duration::from_millis(20),
        }
    }

    pub fn check(&self) -> Result<(), RuntimeError> {
        let positive = self.workers > 0
            && self.task_queue > 0
            && self.event_queue > 0
            && self.global_queue > 0
            && self.batch > 0
            && self.subscriptions > 0;
        let bounded = self.workers <= MAX_WORKERS
            && self.task_queue <= MAX_QUEUE
            && self.event_queue <= MAX_QUEUE
            && self.global_queue <= MAX_QUEUE
            && self.batch <= self.event_queue
            && self.subscriptions <= MAX_SUBSCRIPTIONS
            && self.admission_wait <= MAX_ADMISSION_WAIT;
        if positive && bounded {
            Ok(())
        } else {
            Err(RuntimeError::new(
                "invalid_limit",
                "runtime limits must be positive and within the fixed ceilings",
            ))
        }
    }
}

pub struct Runtime {
    scheduler: Scheduler,
    bus: EventBus,
}

impl Runtime {
    pub fn start(limits: Limits) -> Result<Self, RuntimeError> {
        limits.check()?;
        let scheduler = Scheduler::start(limits.clone())?;
        let bus = EventBus::new(limits)?;
        Ok(Self { scheduler, bus })
    }

    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    pub fn bus(&self) -> &EventBus {
        &self.bus
    }

    /// 先停止任务，再拒绝新的事件。已排队的事件仍可由订阅方取走。
    pub fn shutdown(&self) {
        self.scheduler.shutdown();
        self.bus.close();
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.shutdown();
    }
}
