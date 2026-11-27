//! 比较实测压力和可调整限额，决定保持、暂停或恢复。
//!
//! 缺席的读数保持 absent，不用 0 代替。暂停进程只停止追加记录，不改已经做出的决定。
//! 进入压力暂停，或暂停期间总线丢失增加时，要求宿主记一条丢失标记。

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_REPORT_BYTES: u64 = 1024 * 1024;
const CPU_CEILING: u64 = 1_000_000;
const MEM_CEILING_KIB: u64 = 4_194_304;
const QUEUE_CEILING: u64 = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LimitValue {
    Keep,
    Set(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PluginLimit {
    Keep,
    Clear,
    Set { id: String, queue: u64 },
}

struct Reading {
    cpu_ppm: LimitValue,
    mem_kib: LimitValue,
    queue: LimitValue,
    plugin: PluginLimit,
    rss_kib: Option<u64>,
    cpu_used_ppm: Option<u64>,
    cpu_baseline: bool,
    queued: Option<u64>,
    sched_queued: Option<u64>,
    lost: Option<u64>,
    plugin_queued: Option<u64>,
}

struct Inner {
    cpu_ppm: u64,
    mem_kib: u64,
    queue: u64,
    plugin: Option<String>,
    plugin_queue: u64,
    regulating: bool,
    seen_lost: Option<u64>,
    reports: u64,
}

impl Inner {
    fn standard() -> Self {
        Self {
            cpu_ppm: CPU_CEILING,
            mem_kib: MEM_CEILING_KIB,
            queue: QUEUE_CEILING,
            plugin: None,
            plugin_queue: 0,
            regulating: false,
            seen_lost: None,
            reports: 0,
        }
    }
}

pub struct Session {
    state: PathBuf,
    frame_paused: AtomicBool,
    inner: Mutex<Inner>,
}

impl Session {
    pub fn start(state: &Path) -> Self {
        Self {
            state: state.to_path_buf(),
            frame_paused: AtomicBool::new(false),
            inner: Mutex::new(Inner::standard()),
        }
    }

    pub fn pause(&self) {
        self.frame_paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.frame_paused.store(false, Ordering::Release);
    }

    pub fn regulate(&self, payload: &str) -> Result<String, &'static str> {
        let mut inner = self.inner.lock().map_err(|_| "unavailable")?;
        if self.frame_paused.load(Ordering::Acquire) {
            return Ok(format!(
                "action=keep loss=0 reason=none reports={}",
                inner.reports
            ));
        }
        let reading = parse_reading(payload).ok_or("rejected")?;
        apply_limits(&mut inner, &reading)?;
        let detail = decide(&mut inner, &reading);
        let reports = inner.reports.saturating_add(1);
        let logged = rewrite_reports(&detail, reports);
        if logged.len() > 1024 || logged.chars().any(char::is_control) {
            return Err("rejected");
        }
        append_report(&self.state, &logged).map_err(|_| "unavailable")?;
        inner.reports = reports;
        Ok(logged)
    }

    pub fn release(self) -> io::Result<()> {
        let mut file = open_private(&self.state.join("released"), false)?;
        file.write_all(b"joined")?;
        file.flush()?;
        Ok(())
    }
}

fn rewrite_reports(detail: &str, reports: u64) -> String {
    let mut parts: Vec<String> = detail
        .split(' ')
        .filter(|token| !token.starts_with("reports="))
        .map(str::to_owned)
        .collect();
    parts.push(format!("reports={reports}"));
    parts.join(" ")
}

fn apply_limits(inner: &mut Inner, reading: &Reading) -> Result<(), &'static str> {
    if let LimitValue::Set(value) = reading.cpu_ppm {
        if value > CPU_CEILING {
            return Err("rejected");
        }
        inner.cpu_ppm = value;
    }
    if let LimitValue::Set(value) = reading.mem_kib {
        if value > MEM_CEILING_KIB {
            return Err("rejected");
        }
        inner.mem_kib = value;
    }
    if let LimitValue::Set(value) = reading.queue {
        if value > QUEUE_CEILING {
            return Err("rejected");
        }
        inner.queue = value;
    }
    match &reading.plugin {
        PluginLimit::Keep => {}
        PluginLimit::Clear => {
            inner.plugin = None;
            inner.plugin_queue = 0;
        }
        PluginLimit::Set { id, queue } => {
            if *queue > QUEUE_CEILING {
                return Err("rejected");
            }
            inner.plugin = Some(id.clone());
            inner.plugin_queue = *queue;
        }
    }
    Ok(())
}

fn decide(inner: &mut Inner, reading: &Reading) -> String {
    let queue_depth = fold(reading.queued, reading.sched_queued);
    let queue_over = queue_depth.is_some_and(|depth| depth > inner.queue);
    let memory_over = reading.rss_kib.is_some_and(|rss| rss > inner.mem_kib);
    let cpu_over = reading
        .cpu_used_ppm
        .is_some_and(|used| !reading.cpu_baseline && used > inner.cpu_ppm);
    let plugin_over = match &inner.plugin {
        Some(_) => reading
            .plugin_queued
            .is_some_and(|queued| queued > inner.plugin_queue),
        None => false,
    };
    let reason = if queue_over {
        "queue"
    } else if memory_over {
        "memory"
    } else if cpu_over {
        "cpu"
    } else if plugin_over {
        "plugin"
    } else {
        "none"
    };
    let over = reason != "none";
    let lost_increased = match (inner.seen_lost, reading.lost) {
        (Some(previous), Some(current)) => current > previous,
        _ => false,
    };
    if let Some(lost) = reading.lost {
        inner.seen_lost = Some(lost);
    }
    let (action, loss) = if over {
        let entering = !inner.regulating;
        inner.regulating = true;
        ("pause", entering || lost_increased)
    } else if inner.regulating {
        inner.regulating = false;
        ("resume", false)
    } else {
        ("hold", false)
    };
    detail_line(inner, reading, action, loss, reason)
}

fn detail_line(inner: &Inner, reading: &Reading, action: &str, loss: bool, reason: &str) -> String {
    let mut parts = vec![
        format!("action={action}"),
        format!("loss={}", u8::from(loss)),
        format!("reason={reason}"),
        format!("cpu_ppm={}", inner.cpu_ppm),
        format!("mem_kib={}", inner.mem_kib),
        format!("queue={}", inner.queue),
    ];
    match reading.rss_kib {
        Some(kib) => {
            parts.push("rss=measured".to_owned());
            parts.push(format!("rss_kib={kib}"));
        }
        None => parts.push("rss=absent".to_owned()),
    }
    if reading.cpu_baseline {
        parts.push("cpu=baseline".to_owned());
    } else if let Some(used) = reading.cpu_used_ppm {
        parts.push("cpu=measured".to_owned());
        parts.push(format!("cpu_used_ppm={used}"));
    } else {
        parts.push("cpu=absent".to_owned());
    }
    match (reading.queued, reading.lost) {
        (Some(queued), Some(lost)) => {
            parts.push("bus=present".to_owned());
            parts.push(format!("queued={queued}"));
            parts.push(format!("lost={lost}"));
        }
        _ => parts.push("bus=absent".to_owned()),
    }
    match reading.sched_queued {
        Some(queued) => {
            parts.push("sched=present".to_owned());
            parts.push(format!("sched_queued={queued}"));
        }
        None => parts.push("sched=absent".to_owned()),
    }
    match &inner.plugin {
        Some(id) => {
            parts.push(format!("plugin={id}"));
            parts.push(format!("plugin_queue={}", inner.plugin_queue));
        }
        None => {
            parts.push("plugin=none".to_owned());
            parts.push("plugin_queue=0".to_owned());
        }
    }
    match reading.plugin_queued {
        Some(queued) => {
            parts.push("plugin_state=present".to_owned());
            parts.push(format!("plugin_queued={queued}"));
        }
        None => parts.push("plugin_state=absent".to_owned()),
    }
    parts.join(" ")
}

fn fold(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (None, None) => None,
        (Some(value), None) | (None, Some(value)) => Some(value),
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
    }
}

fn parse_reading(input: &str) -> Option<Reading> {
    if input.is_empty() || input.chars().any(char::is_control) || input.len() > 1024 {
        return None;
    }
    let mut fields = BTreeMap::new();
    for token in input.split(' ') {
        let (key, value) = token.split_once('=')?;
        if key.is_empty() || value.is_empty() || fields.insert(key, value).is_some() {
            return None;
        }
    }
    if fields.get("v")? != &"1" {
        return None;
    }
    let cpu_ppm = parse_limit(fields.get("cpu_ppm")?, CPU_CEILING)?;
    let mem_kib = parse_limit(fields.get("mem_kib")?, MEM_CEILING_KIB)?;
    let queue = parse_limit(fields.get("queue")?, QUEUE_CEILING)?;
    let plugin = parse_plugin(fields.get("plugin")?, fields.get("plugin_queue")?)?;
    let rss_kib = match *fields.get("rss")? {
        "measured" => Some(parse_u64(fields.get("rss_kib")?)?),
        "absent" => {
            if fields.contains_key("rss_kib") {
                return None;
            }
            None
        }
        _ => return None,
    };
    let (cpu_baseline, cpu_used_ppm) = match *fields.get("cpu")? {
        "baseline" => {
            if fields.contains_key("cpu_used_ppm") {
                return None;
            }
            (true, None)
        }
        "measured" => (false, Some(parse_u64(fields.get("cpu_used_ppm")?)?)),
        "absent" => {
            if fields.contains_key("cpu_used_ppm") {
                return None;
            }
            (false, None)
        }
        _ => return None,
    };
    let (queued, lost) = match *fields.get("bus")? {
        "present" => (
            Some(parse_u64(fields.get("queued")?)?),
            Some(parse_u64(fields.get("lost")?)?),
        ),
        "absent" => {
            if fields.contains_key("queued") || fields.contains_key("lost") {
                return None;
            }
            (None, None)
        }
        _ => return None,
    };
    let sched_queued = match *fields.get("sched")? {
        "present" => Some(parse_u64(fields.get("sched_queued")?)?),
        "absent" => {
            if fields.contains_key("sched_queued") {
                return None;
            }
            None
        }
        _ => return None,
    };
    let plugin_queued = match *fields.get("plugin_state")? {
        "present" => Some(parse_u64(fields.get("plugin_queued")?)?),
        "absent" => {
            if fields.contains_key("plugin_queued") {
                return None;
            }
            None
        }
        _ => return None,
    };
    let known = [
        "v",
        "cpu_ppm",
        "mem_kib",
        "queue",
        "plugin",
        "plugin_queue",
        "rss",
        "rss_kib",
        "cpu",
        "cpu_used_ppm",
        "bus",
        "queued",
        "lost",
        "sched",
        "sched_queued",
        "plugin_state",
        "plugin_queued",
    ];
    if fields.keys().any(|key| !known.contains(key)) {
        return None;
    }
    Some(Reading {
        cpu_ppm,
        mem_kib,
        queue,
        plugin,
        rss_kib,
        cpu_used_ppm,
        cpu_baseline,
        queued,
        sched_queued,
        lost,
        plugin_queued,
    })
}

fn parse_limit(value: &str, ceiling: u64) -> Option<LimitValue> {
    if value == "keep" {
        return Some(LimitValue::Keep);
    }
    let number = parse_u64(value)?;
    if number > ceiling {
        return None;
    }
    Some(LimitValue::Set(number))
}

fn parse_plugin(plugin: &str, queue: &str) -> Option<PluginLimit> {
    match plugin {
        "keep" if queue == "keep" => Some(PluginLimit::Keep),
        "none" if queue == "none" => Some(PluginLimit::Clear),
        id if valid_plugin_id(id) => {
            let queue = parse_u64(queue)?;
            if queue > QUEUE_CEILING {
                return None;
            }
            Some(PluginLimit::Set {
                id: id.to_owned(),
                queue,
            })
        }
        _ => None,
    }
}

fn valid_plugin_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 || value == "keep" || value == "none" {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes.iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'.' | b'-')
    }) && !value.contains("..")
        && !value.ends_with('.')
}

fn parse_u64(value: &str) -> Option<u64> {
    if value != "0" && value.as_bytes().first() == Some(&b'0') {
        return None;
    }
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn append_report(state: &Path, line: &str) -> io::Result<()> {
    let path = state.join("regulation.jsonl");
    rotate_if_full(&path)?;
    let mut file = open_private(&path, true)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn rotate_if_full(path: &Path) -> io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < MAX_REPORT_BYTES {
        return Ok(());
    }
    let previous = path
        .parent()
        .unwrap_or(Path::new("."))
        .join("regulation.prev.jsonl");
    let _ = fs::remove_file(&previous);
    fs::rename(path, previous)?;
    Ok(())
}

fn open_private(path: &Path, append: bool) -> io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> String {
        [
            "v=1",
            "cpu_ppm=keep",
            "mem_kib=keep",
            "queue=keep",
            "plugin=none",
            "plugin_queue=none",
            "rss=measured",
            "rss_kib=128",
            "cpu=baseline",
            "bus=present",
            "queued=0",
            "lost=0",
            "sched=present",
            "sched_queued=0",
            "plugin_state=absent",
        ]
        .join(" ")
    }

    fn fields(line: &str) -> BTreeMap<String, String> {
        line.split(' ')
            .map(|token| {
                let (key, value) = token.split_once('=').unwrap();
                (key.to_owned(), value.to_owned())
            })
            .collect()
    }

    fn step(inner: &mut Inner, line: &str) -> BTreeMap<String, String> {
        let reading = parse_reading(line).expect("reading");
        apply_limits(inner, &reading).expect("limits");
        fields(&decide(inner, &reading))
    }

    #[test]
    fn memory_pressure_pauses_once_and_a_raised_limit_resumes() {
        let mut inner = Inner::standard();
        let tight = quiet()
            .replace("mem_kib=keep", "mem_kib=8")
            .replace("rss_kib=128", "rss_kib=9");
        let first = step(&mut inner, &tight);
        assert_eq!(first["action"], "pause");
        assert_eq!(first["loss"], "1");
        assert_eq!(first["reason"], "memory");
        assert_eq!(first["mem_kib"], "8");
        let second = step(&mut inner, &tight);
        assert_eq!(second["action"], "pause");
        assert_eq!(second["loss"], "0");
        let raised = quiet()
            .replace("mem_kib=keep", "mem_kib=9")
            .replace("rss_kib=128", "rss_kib=9");
        let resumed = step(&mut inner, &raised);
        assert_eq!(resumed["action"], "resume");
        assert_eq!(resumed["loss"], "0");
        assert_eq!(resumed["reason"], "none");
    }

    #[test]
    fn queue_and_plugin_pressure_mark_loss_without_using_absent_as_zero() {
        let mut inner = Inner::standard();
        let line = quiet()
            .replace("queued=0", "queued=4")
            .replace("queue=keep", "queue=3");
        let paused = step(&mut inner, &line);
        assert_eq!(paused["reason"], "queue");
        assert_eq!(paused["loss"], "1");
        let mut inner = Inner::standard();
        let absent = quiet()
            .replace("rss=measured rss_kib=128", "rss=absent")
            .replace("mem_kib=keep", "mem_kib=0");
        let held = step(&mut inner, &absent);
        assert_eq!(held["action"], "hold");
        assert_eq!(held["rss"], "absent");
        let mut inner = Inner::standard();
        let targeted = quiet()
            .replace("plugin=none", "plugin=strata.health")
            .replace("plugin_queue=none", "plugin_queue=0")
            .replace(
                "plugin_state=absent",
                "plugin_state=present plugin_queued=2",
            );
        let plugin = step(&mut inner, &targeted);
        assert_eq!(plugin["reason"], "plugin");
        assert_eq!(plugin["loss"], "1");
        assert_eq!(plugin["plugin"], "strata.health");
    }

    #[test]
    fn measured_cpu_pauses_and_baseline_does_not() {
        let mut inner = Inner::standard();
        let held = step(&mut inner, &quiet().replace("cpu_ppm=keep", "cpu_ppm=0"));
        assert_eq!(held["action"], "hold");
        assert_eq!(held["cpu"], "baseline");
        let measured = quiet()
            .replace("cpu=baseline", "cpu=measured cpu_used_ppm=5")
            .replace("cpu_ppm=keep", "cpu_ppm=4");
        let paused = step(&mut inner, &measured);
        assert_eq!(paused["reason"], "cpu");
        assert_eq!(paused["loss"], "1");
        assert_eq!(paused["cpu_used_ppm"], "5");
    }

    #[test]
    fn a_new_bus_loss_while_paused_asks_for_another_marker() {
        let mut inner = Inner::standard();
        let first = quiet()
            .replace("mem_kib=keep", "mem_kib=1")
            .replace("rss_kib=128", "rss_kib=9");
        assert_eq!(step(&mut inner, &first)["loss"], "1");
        let again = quiet()
            .replace("rss_kib=128", "rss_kib=9")
            .replace("lost=0", "lost=3");
        let marked = step(&mut inner, &again);
        assert_eq!(marked["action"], "pause");
        assert_eq!(marked["loss"], "1");
        assert_eq!(marked["reason"], "memory");
    }

    #[test]
    fn padded_unknown_and_mismatched_fields_are_rejected() {
        assert!(parse_reading("").is_none());
        assert!(parse_reading(&quiet().replace("queued=0", "queued=01")).is_none());
        assert!(parse_reading(&format!("{} extra=1", quiet())).is_none());
        assert!(
            parse_reading(&quiet().replace("rss=measured rss_kib=128", "rss=absent rss_kib=1"))
                .is_none()
        );
        assert!(parse_reading(&quiet().replace("cpu=baseline", "cpu=measured")).is_none());
        assert!(
            parse_reading(&quiet().replace("bus=present queued=0 lost=0", "bus=absent queued=1"))
                .is_none()
        );
        assert!(parse_reading(&quiet().replace("plugin=none", "plugin=keep")).is_none());
        assert!(parse_reading(&quiet().replace("cpu_ppm=keep", "cpu_ppm=1000001")).is_none());
        assert!(parse_reading(&quiet().replace("mem_kib=keep", "mem_kib=4194305")).is_none());
        assert!(parse_reading(&quiet().replace("queue=keep", "queue=65537")).is_none());
    }

    #[test]
    fn frame_pause_does_not_append_or_apply_a_tighter_limit() {
        let directory = std::env::temp_dir().join(format!(
            "winstrata-budget-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let session = Session::start(&directory);
        let first = session.regulate(&quiet()).unwrap();
        assert!(first.contains("action=hold"));
        assert!(first.contains("reports=1"));
        session.pause();
        let kept = session
            .regulate(&quiet().replace("mem_kib=keep", "mem_kib=1"))
            .unwrap();
        assert_eq!(kept, "action=keep loss=0 reason=none reports=1");
        let lines = fs::read_to_string(directory.join("regulation.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 1);
        session.resume();
        let paused = session
            .regulate(
                &quiet()
                    .replace("mem_kib=keep", "mem_kib=1")
                    .replace("rss_kib=128", "rss_kib=9"),
            )
            .unwrap();
        assert!(paused.contains("action=pause"));
        assert!(paused.contains("loss=1"));
        assert!(paused.contains("reports=2"));
        session.release().unwrap();
        assert_eq!(fs::read(directory.join("released")).unwrap(), b"joined");
        let _ = fs::remove_dir_all(&directory);
    }
}
