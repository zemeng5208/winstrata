//! Windows 用户态环境变量监视。
//!
//! 只打开范围内的两个环境存储。`RegNotifyChangeKeyValue` 不提供变更时刻，也不提供执行进程。
//! 快照不完整时不编造前后值。PATH 放不进标识符时按分号拆段，不截断完整值。
//! 这里不调用服务控制管理器，也不加载驱动。本文件里的测试只创建易失键，不写入真实的
//! `HKCU\Environment` 或系统 Environment 存储。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{null, null_mut};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_QUERY_VALUE, KEY_WOW64_64KEY,
    REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME, RegCloseKey, RegEnumValueW,
    RegNotifyChangeKeyValue, RegOpenKeyExW,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
};

use crate::{
    DATA_BYTES, EnvError, EnvKey, Hive, Memory, NAME_CHARS, Scope, Session, VALUE_LIMIT,
    ValueChange, ValueSample, diff_values, gap_event, incomplete_event, lock, next_event,
    plan_change, store_prefix,
};
use winstrata_protocols::GapReason;

const ERROR_MORE_DATA: u32 = 234;
const ERROR_NO_MORE_ITEMS: u32 = 259;

pub(crate) struct StopFlag {
    event: HANDLE,
}

unsafe impl Send for StopFlag {}
unsafe impl Sync for StopFlag {}

impl StopFlag {
    fn new() -> Result<Self, EnvError> {
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(EnvError::Os(unsafe { GetLastError() } as i32));
        }
        Ok(Self { event })
    }

    pub(crate) fn signal(&self) {
        unsafe {
            SetEvent(self.event);
        }
    }
}

impl Drop for StopFlag {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.event);
        }
    }
}

struct WinHandle(HANDLE);

impl Drop for WinHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

struct RegKey(HKEY);

impl Drop for RegKey {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
}

struct Armed {
    prefix: &'static str,
    key: RegKey,
    event: WinHandle,
    baseline: Snapshot,
    noted_pause: u64,
    alive: bool,
}

struct Snapshot {
    values: Vec<ValueSample>,
    complete: bool,
}

pub(crate) fn spawn(scope: &Scope, memory: Arc<Mutex<Memory>>) -> Result<Session, EnvError> {
    let keys = scope.keys().to_vec();
    let stop = Arc::new(StopFlag::new()?);
    let stop_for_thread = Arc::clone(&stop);
    let memory_for_thread = Arc::clone(&memory);
    let (ready_tx, ready_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("ws-env".to_owned())
        .spawn(move || {
            let ran = catch_unwind(AssertUnwindSafe(|| {
                watch(&keys, &memory_for_thread, &stop_for_thread, ready_tx)
            }));
            if ran.is_err() {
                let mut guard = lock(&memory_for_thread);
                if guard.fault.is_none() {
                    guard.fault = Some(EnvError::fault("env watch panicked"));
                }
            }
        })
        .map_err(|_| EnvError::fault("env watch did not start"))?;
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(Session {
            memory,
            worker: Some(worker),
            stop: Some(stop),
        }),
        Ok(Err(error)) => {
            stop.signal();
            let _ = worker.join();
            Err(error)
        }
        Err(_) => {
            stop.signal();
            let _ = worker.join();
            Err(EnvError::fault("env watch did not start"))
        }
    }
}

fn watch(
    keys: &[EnvKey],
    memory: &Mutex<Memory>,
    stop: &StopFlag,
    ready: Sender<Result<(), EnvError>>,
) {
    let opened = (|| {
        let mut armed = Vec::with_capacity(keys.len());
        for key in keys {
            armed.push(arm_key(key)?);
        }
        Ok(armed)
    })();
    let mut armed = match opened {
        Ok(armed) => armed,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    for item in &mut armed {
        if unsafe { WaitForSingleObject(item.event.0, 0) } == WAIT_OBJECT_0 {
            // 基线和第一次武装之间的变化不能逐值归因。
            if !next_event(memory, |stream, sequence| {
                gap_event(stream, sequence, GapReason::Lost)
            }) {
                let _ = ready.send(Err(EnvError::fault("env watch stopped")));
                return;
            }
            match snapshot(item.key.0).and_then(|next| {
                item.baseline = next;
                arm(item.key.0, item.event.0)
            }) {
                Ok(()) => {}
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            }
        }
    }
    if ready.send(Ok(())).is_err() {
        return;
    }
    loop {
        let mut handles = vec![stop.event];
        let mut map = Vec::new();
        for (index, item) in armed.iter().enumerate() {
            if item.alive {
                handles.push(item.event.0);
                map.push(index);
            }
        }
        if map.is_empty() {
            break;
        }
        let status =
            unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, INFINITE) };
        if status == WAIT_OBJECT_0 {
            break;
        }
        if status > WAIT_OBJECT_0 && (status as usize) < handles.len() {
            let index = map[(status - WAIT_OBJECT_0 - 1) as usize];
            if !on_key(&mut armed[index], memory) {
                break;
            }
        } else {
            let _ = next_event(memory, |stream, sequence| {
                gap_event(stream, sequence, GapReason::SourceFailure)
            });
            break;
        }
    }
}

fn on_key(item: &mut Armed, memory: &Mutex<Memory>) -> bool {
    if arm(item.key.0, item.event.0).is_err() {
        item.alive = false;
        return next_event(memory, |stream, sequence| {
            gap_event(stream, sequence, GapReason::SourceFailure)
        });
    }
    let now = match snapshot(item.key.0) {
        Ok(now) => now,
        Err(_) => {
            item.alive = false;
            return next_event(memory, |stream, sequence| {
                gap_event(stream, sequence, GapReason::SourceFailure)
            });
        }
    };
    let changes = if item.baseline.complete && now.complete {
        diff_values(&item.baseline.values, &now.values).map_err(|_| ())
    } else {
        Err(())
    };
    let (paused, generation) = {
        let guard = lock(memory);
        (guard.paused, guard.pause_generation)
    };
    // 先推进基线。暂停期间的变化因此不会在恢复后被编成逐值差异。
    item.baseline = now;
    if paused {
        let changed = match &changes {
            Ok(changes) => !changes.is_empty(),
            Err(()) => true,
        };
        if changed && item.noted_pause != generation {
            item.noted_pause = generation;
            next_event(memory, |stream, sequence| {
                gap_event(stream, sequence, GapReason::Paused)
            })
        } else {
            true
        }
    } else {
        match &changes {
            Ok(changes) if changes.is_empty() => true,
            Ok(changes) => emit_values(memory, item.prefix, changes),
            Err(()) => next_event(memory, |stream, sequence| {
                incomplete_event(stream, sequence)
            }),
        }
    }
}

fn emit_values(memory: &Mutex<Memory>, prefix: &str, changes: &[ValueChange]) -> bool {
    for change in changes {
        for planned in plan_change(prefix, change) {
            let event_type = planned.event_type;
            let subject = planned.subject;
            let evidence = planned.evidence;
            let partial = planned.partial;
            if !next_event(memory, move |stream, sequence| {
                crate::value_observation(
                    stream,
                    sequence,
                    event_type,
                    subject.as_deref(),
                    &evidence,
                    partial,
                )
            }) {
                return false;
            }
        }
    }
    true
}

fn arm_key(key: &EnvKey) -> Result<Armed, EnvError> {
    let root = match key.hive {
        Hive::CurrentUser => HKEY_CURRENT_USER,
        Hive::LocalMachine => HKEY_LOCAL_MACHINE,
    };
    let opened = open_key(root, &key.subkey)?;
    let event = WinHandle(event_handle()?);
    let baseline = snapshot(opened.0)?;
    arm(opened.0, event.0)?;
    Ok(Armed {
        prefix: store_prefix(key.hive),
        key: opened,
        event,
        baseline,
        noted_pause: 0,
        alive: true,
    })
}

fn open_key(root: HKEY, subkey: &str) -> Result<RegKey, EnvError> {
    let name = wide(subkey)?;
    let mut key = null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            root,
            name.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_NOTIFY | KEY_WOW64_64KEY,
            &mut key,
        )
    };
    if status != 0 {
        return Err(EnvError::Os(status as i32));
    }
    Ok(RegKey(key))
}

fn arm(key: HKEY, event: HANDLE) -> Result<(), EnvError> {
    unsafe { ResetEvent(event) };
    let status = unsafe {
        RegNotifyChangeKeyValue(
            key,
            0,
            REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
            event,
            1,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(EnvError::Os(status as i32))
    }
}

fn snapshot(key: HKEY) -> Result<Snapshot, EnvError> {
    let mut values = Vec::new();
    let mut complete = true;
    let mut index = 0u32;
    loop {
        if values.len() == VALUE_LIMIT {
            complete = false;
            break;
        }
        // 名称上限是 256 个码元，缓冲区再留一个终止 NUL。
        let mut name = vec![0u16; NAME_CHARS + 1];
        let mut name_chars = name.len() as u32;
        let mut kind = 0u32;
        let mut data = vec![0u8; DATA_BYTES];
        let mut data_len = data.len() as u32;
        let status = unsafe {
            RegEnumValueW(
                key,
                index,
                name.as_mut_ptr(),
                &mut name_chars,
                null_mut(),
                &mut kind,
                data.as_mut_ptr(),
                &mut data_len,
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status == ERROR_MORE_DATA {
            complete = false;
            break;
        }
        if status != 0 {
            return Err(EnvError::Os(status as i32));
        }
        if name_chars as usize > NAME_CHARS || data_len as usize > DATA_BYTES {
            complete = false;
            break;
        }
        let name = match String::from_utf16(&name[..name_chars as usize]) {
            Ok(name) => name,
            Err(_) => {
                complete = false;
                break;
            }
        };
        if values
            .iter()
            .any(|sample: &ValueSample| sample.name.eq_ignore_ascii_case(&name))
        {
            complete = false;
            break;
        }
        values.push(ValueSample {
            name,
            kind,
            data: data[..data_len as usize].to_vec(),
        });
        index += 1;
    }
    Ok(Snapshot { values, complete })
}

fn event_handle() -> Result<HANDLE, EnvError> {
    let event = unsafe { CreateEventW(null(), 1, 0, null()) };
    if event.is_null() {
        Err(EnvError::Os(unsafe { GetLastError() } as i32))
    } else {
        Ok(event)
    }
}

fn wide(text: &str) -> Result<Vec<u16>, EnvError> {
    if text.chars().any(|symbol| symbol == '\0') {
        return Err(EnvError::fault("invalid env key"));
    }
    Ok(text.encode_utf16().chain(std::iter::once(0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;
    use std::time::Instant;

    use windows_sys::Win32::System::Registry::{
        KEY_SET_VALUE, REG_OPTION_VOLATILE, REG_SZ, RegCreateKeyExW, RegDeleteTreeW,
        RegDeleteValueW, RegSetValueExW,
    };
    use winstrata_protocols::{Attribution, Completeness, Event, Record};

    use crate::{PLUGIN_ID, PROVIDER, Scope};

    fn serialize() -> MutexGuard<'static, ()> {
        static GUARD: Mutex<()> = Mutex::new(());
        GUARD.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    struct Probe {
        hive: HKEY,
        subkey: String,
        wide: Vec<u16>,
        write: HKEY,
    }

    impl Probe {
        fn create(hive: HKEY, subkey: &str) -> Self {
            assert!(
                subkey != "Environment"
                    && !subkey.ends_with(r"\Environment")
                    && !subkey.contains(r"Session Manager\Environment"),
                "refusing to write a real environment store: {subkey}"
            );
            let wide = wide(subkey).unwrap();
            let mut write = null_mut();
            let status = unsafe {
                RegCreateKeyExW(
                    hive,
                    wide.as_ptr(),
                    0,
                    null(),
                    REG_OPTION_VOLATILE,
                    KEY_QUERY_VALUE | KEY_SET_VALUE | KEY_WOW64_64KEY,
                    null(),
                    &mut write,
                    null_mut(),
                )
            };
            assert_eq!(status, 0, "create {subkey} failed with {status}");
            Self {
                hive,
                subkey: subkey.to_owned(),
                wide,
                write,
            }
        }

        fn set_sz(&self, name: &str, value: &str) {
            let name = wide(name).unwrap();
            let encoded: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = (encoded.len() * 2) as u32;
            let status = unsafe {
                RegSetValueExW(
                    self.write,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    encoded.as_ptr().cast(),
                    bytes,
                )
            };
            assert_eq!(status, 0, "set failed with {status}");
        }

        fn delete_value(&self, name: &str) {
            let name = wide(name).unwrap();
            let status = unsafe { RegDeleteValueW(self.write, name.as_ptr()) };
            assert_eq!(status, 0, "delete failed with {status}");
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            unsafe {
                if !self.write.is_null() {
                    RegCloseKey(self.write);
                    self.write = null_mut();
                }
                RegDeleteTreeW(self.hive, self.wide.as_ptr());
            }
        }
    }

    fn evidence(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation {
                attribution: Attribution::Observed { evidence },
                ..
            } => Some(evidence.as_str()),
            _ => None,
        }
    }

    fn subject(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation {
                subject: Some(subject),
                ..
            } => Some(subject.id.as_str()),
            _ => None,
        }
    }

    fn event_type(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation { event_type, .. } => Some(event_type.as_str()),
            _ => None,
        }
    }

    fn is_paused(event: &Event) -> bool {
        matches!(
            &event.record,
            Record::Gap {
                reason: GapReason::Paused,
                ..
            }
        )
    }

    fn is_value(event: &Event) -> bool {
        matches!(
            event_type(event),
            Some(
                "env.variable_added"
                    | "env.variable_removed"
                    | "env.variable_changed"
                    | "env.path_added"
                    | "env.path_removed"
                    | "env.path_changed"
            )
        )
    }

    fn wait_until(session: &Session, mut found: impl FnMut(&Event) -> bool) -> Vec<Event> {
        let started = Instant::now();
        loop {
            let events = session.query();
            if events.iter().any(|event| found(event)) || started.elapsed() > Duration::from_secs(5)
            {
                return events;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn assert_provenance(event: &Event, user: &str) {
        assert_eq!(event.source.device.as_str(), PLUGIN_ID);
        assert_eq!(event.source.provider.as_str(), PROVIDER);
        assert!(event.source.epoch.as_str().starts_with("env-"));
        assert!(event.time.precision_ns.is_none());
        match &event.record {
            Record::Observation {
                process,
                user: observed,
                attribution: Attribution::Observed { .. },
                ..
            } => {
                assert!(process.is_none());
                assert_eq!(observed.as_ref().map(|id| id.as_str()), Some(user));
            }
            Record::Gap { .. } => {}
            other => panic!("unexpected record {other:?}"),
        }
    }

    fn pair(suffix: &str) -> (Probe, Probe, Scope) {
        let pid = std::process::id();
        let hkcu = Probe::create(
            HKEY_CURRENT_USER,
            &format!(r"Software\WinStrataEnv020{suffix}u{pid}"),
        );
        let hklm = Probe::create(
            HKEY_LOCAL_MACHINE,
            &format!(r"SOFTWARE\WinStrataEnv020{suffix}m{pid}"),
        );
        let scope = Scope::from_pairs(&[
            ("HKCU", hkcu.subkey.as_str()),
            ("HKLM", hklm.subkey.as_str()),
        ])
        .unwrap();
        (hkcu, hklm, scope)
    }

    #[test]
    fn volatile_user_and_system_values_keep_before_and_after() {
        let _guard = serialize();
        match Session::start(&Scope::product(), Some("env-user"), 0, None) {
            Err(error) => assert_eq!(error.detail(), "invalid env epoch"),
            Ok(_) => panic!("epoch zero started a watch"),
        }
        let (hkcu, hklm, scope) = pair("v");
        hkcu.set_sz("Probe", "alpha");
        hklm.set_sz("Probe", "alpha");
        let mut session = Session::start(&scope, Some("env-user"), 2, None).unwrap();
        assert!(
            session
                .query()
                .iter()
                .all(|event| !matches!(event.record, Record::Observation { .. })),
            "baseline was published: {:?}",
            session.query()
        );

        hkcu.set_sz("Probe", "beta");
        let events = wait_until(&session, |event| {
            subject(event).is_some_and(|id| id.starts_with("user\\"))
                && evidence(event) == Some("t=sz;b=616c706861;a=62657461")
        });
        let changed = events
            .iter()
            .find(|event| evidence(event) == Some("t=sz;b=616c706861;a=62657461"))
            .unwrap_or_else(|| panic!("missing user change: {events:?}"));
        assert_eq!(event_type(changed), Some("env.variable_changed"));
        assert_eq!(subject(changed), Some("user\\Probe"));
        assert!(!subject(changed).unwrap().contains("HKCU"));
        assert!(!subject(changed).unwrap().contains(&hkcu.subkey));
        assert_provenance(changed, "env-user");

        hklm.set_sz("Probe", "gamma");
        let events = wait_until(&session, |event| {
            evidence(event) == Some("t=sz;b=616c706861;a=67616d6d61")
        });
        let changed = events
            .iter()
            .find(|event| evidence(event) == Some("t=sz;b=616c706861;a=67616d6d61"))
            .unwrap_or_else(|| panic!("missing system change: {events:?}"));
        assert_eq!(event_type(changed), Some("env.variable_changed"));
        assert_eq!(subject(changed), Some("system\\Probe"));
        assert!(!subject(changed).unwrap().contains("HKLM"));
        assert_provenance(changed, "env-user");

        hklm.set_sz("Extra", "one");
        let events = wait_until(&session, |event| evidence(event) == Some("t=sz;a=6f6e65"));
        let added = events
            .iter()
            .find(|event| evidence(event) == Some("t=sz;a=6f6e65"))
            .unwrap_or_else(|| panic!("missing add: {events:?}"));
        assert_eq!(event_type(added), Some("env.variable_added"));
        assert_eq!(subject(added), Some("system\\Extra"));

        hklm.delete_value("Extra");
        let events = wait_until(&session, |event| evidence(event) == Some("t=sz;b=6f6e65"));
        let removed = events
            .iter()
            .find(|event| evidence(event) == Some("t=sz;b=6f6e65"))
            .unwrap_or_else(|| panic!("missing removal: {events:?}"));
        assert_eq!(event_type(removed), Some("env.variable_removed"));
        assert_eq!(subject(removed), Some("system\\Extra"));
        assert!(matches!(
            &removed.record,
            Record::Observation {
                completeness: Completeness::Complete,
                ..
            }
        ));
        session.stop();
    }

    #[test]
    fn pause_publishes_one_gap_and_advances_the_baseline() {
        let _guard = serialize();
        let (hkcu, hklm, scope) = pair("p");
        hkcu.set_sz("Probe", "alpha");
        hklm.set_sz("Probe", "alpha");
        let session = Session::start(&scope, Some("env-user"), 5, None).unwrap();
        session.pause().unwrap();
        hkcu.set_sz("Probe", "beta");
        let events = wait_until(&session, is_paused);
        assert_eq!(events.iter().filter(|event| is_paused(event)).count(), 1);
        assert!(events.iter().all(|event| !is_value(event)));
        thread::sleep(Duration::from_millis(100));
        hkcu.set_sz("Probe", "later");
        thread::sleep(Duration::from_millis(500));
        let during = session.query();
        assert_eq!(during.iter().filter(|event| is_paused(event)).count(), 1);
        assert!(during.iter().all(|event| !is_value(event)));
        session.resume().unwrap();
        hkcu.set_sz("Probe", "gamma");
        let events = wait_until(&session, |event| {
            evidence(event) == Some("t=sz;b=6c61746572;a=67616d6d61")
        });
        let changed = events
            .iter()
            .find(|event| evidence(event) == Some("t=sz;b=6c61746572;a=67616d6d61"))
            .unwrap_or_else(|| panic!("baseline was not advanced: {events:?}"));
        assert_eq!(event_type(changed), Some("env.variable_changed"));
        assert_eq!(subject(changed), Some("user\\Probe"));
        assert_eq!(events.iter().filter(|event| is_paused(event)).count(), 1);
        assert!(!events.iter().any(|event| {
            evidence(event)
                .is_some_and(|text| text.contains("616c706861") && text.contains("67616d6d61"))
        }));
    }

    #[test]
    fn an_incomplete_snapshot_does_not_invent_a_value_diff() {
        let _guard = serialize();
        let (hkcu, hklm, scope) = pair("i");
        for index in 0..VALUE_LIMIT {
            hkcu.set_sz(&format!("n{index:03}"), "x");
        }
        hklm.set_sz("Keep", "1");
        let session = Session::start(&scope, Some("env-user"), 6, None).unwrap();
        hkcu.set_sz("n000", "y");
        let events = wait_until(&session, |event| {
            event_type(event) == Some("env.block_changed")
        });
        assert!(events.iter().any(|event| {
            matches!(
                &event.record,
                Record::Observation {
                    subject: None,
                    completeness: Completeness::Partial { reason },
                    attribution: Attribution::Observed { evidence },
                    ..
                } if reason.as_str() == "snapshot_incomplete" && evidence.as_str() == PROVIDER
            )
        }));
        assert!(events.iter().all(|event| !is_value(event)));
    }

    #[test]
    fn a_change_while_stopped_is_not_replayed() {
        let _guard = serialize();
        let (hkcu, hklm, scope) = pair("s");
        hkcu.set_sz("Probe", "alpha");
        hklm.set_sz("Probe", "alpha");
        let mut session = Session::start(&scope, Some("env-user"), 7, None).unwrap();
        hkcu.set_sz("Probe", "beta");
        let events = wait_until(&session, |event| {
            evidence(event) == Some("t=sz;b=616c706861;a=62657461")
        });
        assert!(
            events
                .iter()
                .any(|event| { evidence(event) == Some("t=sz;b=616c706861;a=62657461") })
        );
        let before_stop = session.query().len();
        session.stop();
        hkcu.set_sz("Probe", "gamma");
        thread::sleep(Duration::from_millis(300));
        assert_eq!(session.query().len(), before_stop);

        let mut again = Session::start(&scope, Some("env-user"), 8, None).unwrap();
        assert!(
            again
                .query()
                .iter()
                .all(|event| !matches!(event.record, Record::Observation { .. })),
            "stopped change was replayed: {:?}",
            again.query()
        );
        hklm.set_sz("Later", "one");
        let events = wait_until(&again, |event| evidence(event) == Some("t=sz;a=6f6e65"));
        assert!(events.iter().any(|event| {
            evidence(event) == Some("t=sz;a=6f6e65") && subject(event) == Some("system\\Later")
        }));
        assert!(
            !events
                .iter()
                .any(|event| { evidence(event).is_some_and(|text| text.contains("67616d6d61")) })
        );
        again.stop();
    }
}
