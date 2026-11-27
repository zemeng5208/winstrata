//! Windows 用户态注册表监视。
//!
//! `RegNotifyChangeKeyValue` 不提供变更时刻，也不提供执行进程。快照不完整时不编造前后值。
//! 这里不调用服务控制管理器，也不加载驱动。

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
    DATA_BYTES, Hive, Memory, NAME_CHARS, RegistryError, RegistryKey, Scope, Session, VALUE_LIMIT,
    ValueChange, ValueSample, diff_values, gap_event, incomplete_event, lock, next_event,
    value_event,
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
    fn new() -> Result<Self, RegistryError> {
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(RegistryError::Os(unsafe { GetLastError() } as i32));
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
    hive: &'static str,
    subkey: String,
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

pub(crate) fn spawn(scope: &Scope, memory: Arc<Mutex<Memory>>) -> Result<Session, RegistryError> {
    let keys = scope.keys().to_vec();
    let stop = Arc::new(StopFlag::new()?);
    let stop_for_thread = Arc::clone(&stop);
    let memory_for_thread = Arc::clone(&memory);
    let (ready_tx, ready_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("ws-registry".to_owned())
        .spawn(move || {
            let ran = catch_unwind(AssertUnwindSafe(|| {
                watch(&keys, &memory_for_thread, &stop_for_thread, ready_tx)
            }));
            if ran.is_err() {
                let mut guard = lock(&memory_for_thread);
                if guard.fault.is_none() {
                    guard.fault = Some(RegistryError::fault("registry watch panicked"));
                }
            }
        })
        .map_err(|_| RegistryError::fault("registry watch did not start"))?;
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
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
            Err(RegistryError::fault("registry watch did not start"))
        }
    }
}

fn watch(
    keys: &[RegistryKey],
    memory: &Mutex<Memory>,
    stop: &StopFlag,
    ready: Sender<Result<(), RegistryError>>,
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
                let _ = ready.send(Err(RegistryError::fault("registry watch stopped")));
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
    let continued = if paused {
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
            Ok(changes) => emit_values(memory, item.hive, &item.subkey, changes),
            Err(()) => next_event(memory, |stream, sequence| {
                incomplete_event(stream, sequence)
            }),
        }
    };
    item.baseline = now;
    continued
}

fn emit_values(memory: &Mutex<Memory>, hive: &str, subkey: &str, changes: &[ValueChange]) -> bool {
    for change in changes {
        let hive = hive.to_owned();
        let subkey = subkey.to_owned();
        let change = change.clone();
        if !next_event(memory, move |stream, sequence| {
            value_event(stream, sequence, &hive, &subkey, &change)
        }) {
            return false;
        }
    }
    true
}

fn arm_key(key: &RegistryKey) -> Result<Armed, RegistryError> {
    let root = match key.hive {
        Hive::CurrentUser => HKEY_CURRENT_USER,
        Hive::LocalMachine => HKEY_LOCAL_MACHINE,
    };
    let opened = open_key(root, &key.subkey)?;
    let event = WinHandle(event_handle()?);
    let baseline = snapshot(opened.0)?;
    arm(opened.0, event.0)?;
    Ok(Armed {
        hive: key.hive.as_str(),
        subkey: key.subkey.clone(),
        key: opened,
        event,
        baseline,
        noted_pause: 0,
        alive: true,
    })
}

fn open_key(root: HKEY, subkey: &str) -> Result<RegKey, RegistryError> {
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
        return Err(RegistryError::Os(status as i32));
    }
    Ok(RegKey(key))
}

fn arm(key: HKEY, event: HANDLE) -> Result<(), RegistryError> {
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
        Err(RegistryError::Os(status as i32))
    }
}

fn snapshot(key: HKEY) -> Result<Snapshot, RegistryError> {
    let mut values = Vec::new();
    let mut complete = true;
    let mut index = 0u32;
    loop {
        if values.len() == VALUE_LIMIT {
            complete = false;
            break;
        }
        let mut name = vec![0u16; NAME_CHARS];
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
            return Err(RegistryError::Os(status as i32));
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
            .any(|sample: &ValueSample| sample.name == name)
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

fn event_handle() -> Result<HANDLE, RegistryError> {
    let event = unsafe { CreateEventW(null(), 1, 0, null()) };
    if event.is_null() {
        Err(RegistryError::Os(unsafe { GetLastError() } as i32))
    } else {
        Ok(event)
    }
}

fn wide(text: &str) -> Result<Vec<u16>, RegistryError> {
    if text.chars().any(|symbol| symbol == '\0') {
        return Err(RegistryError::fault("invalid registry key"));
    }
    Ok(text.encode_utf16().chain(std::iter::once(0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use windows_sys::Win32::System::Registry::{
        KEY_SET_VALUE, REG_DWORD, REG_OPTION_VOLATILE, RegCreateKeyExW, RegDeleteTreeW,
        RegDeleteValueW, RegSetValueExW,
    };
    use winstrata_protocols::{Attribution, Completeness, Event, Record};

    use crate::{PLUGIN_ID, PROVIDER, Scope};

    struct Probe {
        hive: HKEY,
        subkey: String,
        wide: Vec<u16>,
        write: HKEY,
    }

    impl Probe {
        fn create(hive: HKEY, subkey: &str) -> Self {
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

        fn set_dword(&self, name: &str, value: u32) {
            let name = wide(name).unwrap();
            let bytes = value.to_le_bytes();
            let status = unsafe {
                RegSetValueExW(self.write, name.as_ptr(), 0, REG_DWORD, bytes.as_ptr(), 4)
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
        assert!(event.source.epoch.as_str().starts_with("reg-"));
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

    #[test]
    fn product_keys_open_and_the_baseline_is_not_a_change() {
        let session = Session::start(&Scope::product(), Some("registry-user"), 1, None)
            .expect("product registry scope");
        for event in session.query() {
            assert_provenance(&event, "registry-user");
            assert_ne!(event_type(&event), Some("registry.value_added"));
            assert_ne!(event_type(&event), Some("registry.value_changed"));
            assert_ne!(event_type(&event), Some("registry.value_removed"));
        }
    }

    #[test]
    fn current_user_and_system_values_keep_before_and_after() {
        let pid = std::process::id();
        let hkcu = Probe::create(
            HKEY_CURRENT_USER,
            &format!(r"Software\WinStrataProbe019u{pid}"),
        );
        let hklm = Probe::create(
            HKEY_LOCAL_MACHINE,
            &format!(r"SOFTWARE\WinStrataProbe019m{pid}"),
        );
        hkcu.set_dword("Probe", 0x11);
        hklm.set_dword("Probe", 0x11);
        let scope = Scope::from_pairs(&[
            ("HKCU", hkcu.subkey.as_str()),
            ("HKLM", hklm.subkey.as_str()),
        ])
        .unwrap();
        let mut session = Session::start(&scope, Some("registry-user"), 2, None).unwrap();
        assert!(
            session
                .query()
                .iter()
                .all(|event| event_type(event).is_none()),
            "baseline was published: {:?}",
            session.query()
        );

        hkcu.set_dword("Probe", 0x22);
        let events = wait_until(&session, |event| {
            subject(event).is_some_and(|id| id.contains(&hkcu.subkey))
                && evidence(event) == Some("t=4;b=11000000;a=22000000")
        });
        let changed = events
            .iter()
            .find(|event| evidence(event) == Some("t=4;b=11000000;a=22000000"))
            .unwrap_or_else(|| panic!("missing HKCU change: {events:?}"));
        assert_eq!(event_type(changed), Some("registry.value_changed"));
        let expected = format!(r"HKCU\{}\Probe", hkcu.subkey);
        assert_eq!(subject(changed), Some(expected.as_str()));
        assert_provenance(changed, "registry-user");

        hklm.set_dword("Probe", 0x33);
        let events = wait_until(&session, |event| {
            evidence(event) == Some("t=4;b=11000000;a=33000000")
        });
        let changed = events
            .iter()
            .find(|event| evidence(event) == Some("t=4;b=11000000;a=33000000"))
            .unwrap_or_else(|| panic!("missing HKLM change: {events:?}"));
        assert_eq!(
            subject(changed).unwrap(),
            format!(r"HKLM\{}\Probe", hklm.subkey)
        );
        assert_provenance(changed, "registry-user");

        hklm.set_dword("Extra", 0x44);
        let events = wait_until(&session, |event| evidence(event) == Some("t=4;a=44000000"));
        let added = events
            .iter()
            .find(|event| evidence(event) == Some("t=4;a=44000000"))
            .unwrap_or_else(|| panic!("missing add: {events:?}"));
        assert_eq!(event_type(added), Some("registry.value_added"));
        assert!(subject(added).unwrap().starts_with("HKLM\\"));

        hklm.delete_value("Extra");
        let events = wait_until(&session, |event| evidence(event) == Some("t=4;b=44000000"));
        let removed = events
            .iter()
            .find(|event| evidence(event) == Some("t=4;b=44000000"))
            .unwrap_or_else(|| panic!("missing removal: {events:?}"));
        assert_eq!(event_type(removed), Some("registry.value_removed"));
        assert!(matches!(
            &removed.record,
            Record::Observation {
                completeness: Completeness::Complete,
                ..
            }
        ));

        let before_stop = session.query().len();
        session.stop();
        hklm.set_dword("Probe", 0x55);
        thread::sleep(Duration::from_millis(300));
        assert_eq!(session.query().len(), before_stop);

        let mut again = Session::start(&scope, Some("registry-user"), 3, None).unwrap();
        assert!(
            again
                .query()
                .iter()
                .all(|event| event_type(event).is_none()),
            "stopped change was replayed: {:?}",
            again.query()
        );
        hkcu.set_dword("Later", 0x66);
        let events = wait_until(&again, |event| evidence(event) == Some("t=4;a=66000000"));
        assert!(events.iter().any(|event| {
            evidence(event) == Some("t=4;a=66000000")
                && subject(event).is_some_and(|id| id.starts_with("HKCU\\"))
        }));
        assert!(
            !events
                .iter()
                .any(|event| { evidence(event).is_some_and(|text| text.contains("55000000")) })
        );
        again.stop();
    }

    #[test]
    fn an_incomplete_snapshot_does_not_invent_a_value_diff() {
        let pid = std::process::id();
        let hkcu = Probe::create(
            HKEY_CURRENT_USER,
            &format!(r"Software\WinStrataProbe019i{pid}"),
        );
        let hklm = Probe::create(
            HKEY_LOCAL_MACHINE,
            &format!(r"SOFTWARE\WinStrataProbe019i{pid}"),
        );
        for index in 0..VALUE_LIMIT {
            hkcu.set_dword(&format!("n{index:02}"), index as u32);
        }
        hklm.set_dword("Keep", 1);
        let scope = Scope::from_pairs(&[
            ("HKCU", hkcu.subkey.as_str()),
            ("HKLM", hklm.subkey.as_str()),
        ])
        .unwrap();
        let session = Session::start(&scope, Some("registry-user"), 4, None).unwrap();
        hkcu.set_dword("n00", 99);
        let events = wait_until(&session, |event| {
            event_type(event) == Some("registry.key_changed")
        });
        assert!(events.iter().any(|event| {
            matches!(
                &event.record,
                Record::Observation {
                    completeness: Completeness::Partial { reason },
                    ..
                } if reason.as_str() == "snapshot_incomplete"
            )
        }));
        assert!(events.iter().all(|event| {
            !matches!(
                event_type(event),
                Some("registry.value_added" | "registry.value_changed" | "registry.value_removed")
            )
        }));
    }
}
