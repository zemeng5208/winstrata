//! 计划 025 的自身诊断快照。
//!
//! 只读取宿主已经附着的总线和调度器计数，以及插件条目里的故障状态。
//! 不发布事件，不改队列，也不启动其他采集器。

use std::path::{Path, PathBuf};

use crate::bus::BusStats;
use crate::schedule::SchedulerStats;

pub(super) const PLUGIN: &str = "strata.health";

// 清单与诊断程序共同描述计划 025。宿主不通过第三方安装写入这个身份。
pub(super) const MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.health","name":"StrataHealth","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["health.observe"],"permissions":["storage.private"],"#,
    r#""dependencies":[],"events":[],"rescue":false}"#,
);

pub(super) fn beside(runtime: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "winstrata-strata-health.exe"
    } else {
        "winstrata-strata-health"
    };
    let path = runtime.parent()?.join(name);
    path.is_file().then_some(path)
}

/// 帧文本字段最长 1,024 字节。这里只放十进制计数，调用方在发送前检查长度。
pub(super) fn snapshot_text(
    sent_ns: i64,
    plugin_faults: u64,
    bus: Option<BusStats>,
    scheduler: Option<SchedulerStats>,
) -> String {
    let mut parts = Vec::with_capacity(16);
    parts.push("v=1".to_owned());
    parts.push(format!("sent_ns={sent_ns}"));
    parts.push(format!("plugin_faults={plugin_faults}"));
    match bus {
        Some(stats) => {
            parts.push("bus=present".to_owned());
            parts.push(format!("bus_queued={}", count(stats.queued)));
            parts.push(format!("bus_pending={}", count(stats.pending)));
            parts.push(format!("bus_global={}", count(stats.global_queued)));
            parts.push(format!("lost={}", stats.lost));
            parts.push(format!("filtered={}", stats.filtered));
            parts.push(format!("queue_rejects={}", stats.queue_rejects));
        }
        None => parts.push("bus=absent".to_owned()),
    }
    match scheduler {
        Some(stats) => {
            parts.push("sched=present".to_owned());
            parts.push(format!("sched_queued={}", count(stats.queued)));
            parts.push(format!("sched_inflight={}", count(stats.inflight)));
            parts.push(format!("sched_rejected={}", stats.rejected));
            parts.push(format!("sched_timed_out={}", stats.timed_out));
            parts.push(format!("sched_panicked={}", stats.panicked));
        }
        None => parts.push("sched=absent".to_owned()),
    }
    parts.join(" ")
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widest_snapshot_fits_a_plugin_frame() {
        let bus = BusStats {
            accepted: u64::MAX,
            duplicate_rejects: u64::MAX,
            missing_rejects: u64::MAX,
            invalid_rejects: u64::MAX,
            queue_rejects: u64::MAX,
            lost: u64::MAX,
            filtered: u64::MAX,
            released: u64::MAX,
            queued: usize::MAX,
            pending: usize::MAX,
            subscribers: usize::MAX,
            global_queued: usize::MAX,
        };
        let scheduler = SchedulerStats {
            submitted: u64::MAX,
            finished: u64::MAX,
            cancelled: u64::MAX,
            timed_out: u64::MAX,
            rejected: u64::MAX,
            panicked: u64::MAX,
            queued: usize::MAX,
            inflight: usize::MAX,
            live_threads: usize::MAX,
        };
        let text = snapshot_text(i64::MAX, u64::MAX, Some(bus), Some(scheduler));
        assert!(text.len() <= 1024, "snapshot length {}", text.len());
        assert!(!text.chars().any(char::is_control));
        assert!(text.starts_with("v=1 "));
        assert!(text.contains("bus=present"));
        assert!(text.contains("sched=present"));
    }

    #[test]
    fn missing_runtime_counters_stay_absent() {
        let text = snapshot_text(0, 2, None, None);
        assert_eq!(
            text,
            "v=1 sent_ns=0 plugin_faults=2 bus=absent sched=absent"
        );
    }
}
