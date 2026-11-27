use super::{Core, RunState, Settings, StorageState, platform};
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("winstrata-core-{}", platform::fresh_id().unwrap()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn directory(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn saved_location(locator: &Path) -> Vec<u8> {
    fs::read(locator.join("WinStrata/location.json")).unwrap()
}

fn needs_directory(core: &Core) {
    assert!(matches!(
        core.storage_state(),
        StorageState::NeedsDirectory { .. }
    ));
}

#[test]
fn first_selection_run_state_and_settings_survive_the_expected_boundaries() {
    let temp = Scratch::new();
    let locator = temp.directory("locator");
    let root = temp.directory("data root");
    let watched = temp.directory("watch");
    let mut core = Core::open_at(&locator, None).unwrap();
    needs_directory(&core);
    assert!(core.settings().is_err());
    assert!(core.start_live().is_err());
    core.select_directory(&root).unwrap();
    assert!(matches!(core.storage_state(), StorageState::Ready));
    assert_eq!(core.current_user(), platform::current_id().unwrap());
    let defaults = core.settings().unwrap();
    assert!(!defaults.start_at_login && defaults.monitored_roots.is_empty());
    assert!(core.start_live().is_err());
    assert_eq!(core.run_state(), RunState::Stopped);
    let settings = Settings {
        start_at_login: true,
        monitored_roots: vec![watched],
    };
    core.update_settings(&settings).unwrap();
    assert_eq!(core.run_state(), RunState::Stopped);
    core.start_live().unwrap();
    assert_eq!(core.run_state(), RunState::Live);
    core.enter_history();
    assert_eq!(core.run_state(), RunState::History);
    core.stop();
    assert_eq!(core.run_state(), RunState::Stopped);
    core.start_live().unwrap();
    drop(core);

    let mut reopened = Core::open_at(&locator, None).unwrap();
    assert_eq!(reopened.run_state(), RunState::Stopped);
    assert_eq!(reopened.selected_root(), Some(root.as_path()));
    assert_eq!(reopened.settings().unwrap(), settings);
    assert!(reopened.check_capacity(1).is_ok());
    reopened.start_live().unwrap();
    assert_eq!(
        reopened.check_capacity(u64::MAX).unwrap_err().kind(),
        std::io::ErrorKind::StorageFull
    );
    needs_directory(&reopened);
    assert_eq!(reopened.run_state(), RunState::Stopped);
    reopened.select_directory(&root).unwrap();
    assert_eq!(reopened.settings().unwrap(), settings);
}

#[test]
fn concurrent_session_lock_releases_after_process_exit() {
    const CHILD_LOCATOR: &str = "WINSTRATA_TEST_LOCK_HOLDER_LOCATOR";
    if let Some(path) = std::env::var_os(CHILD_LOCATOR) {
        let locator = PathBuf::from(path);
        let _held = Core::open_at(&locator, None).unwrap();
        fs::write(locator.join("holder-ready"), b"ready").unwrap();
        let mut release = [0u8];
        std::io::stdin().read_exact(&mut release).unwrap();
        // 直接终止该测试子进程，操作系统负责释放锁，不调用 Core 的析构函数。
        std::process::exit(0);
    }

    let temp = Scratch::new();
    let locator = temp.directory("locator");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::concurrent_session_lock_releases_after_process_exit",
        ])
        .env(CHILD_LOCATOR, &locator)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let timeout = Instant::now() + Duration::from_secs(15);
    while !locator.join("holder-ready").is_file() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "lock holder exited early"
        );
        if Instant::now() >= timeout {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("timed out waiting for the lock holder");
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let locked = Core::open_at(&locator, None).is_err();
    child.stdin.take().unwrap().write_all(&[1]).unwrap();
    assert!(child.wait().unwrap().success());
    assert!(locked);
    Core::open_at(&locator, None).unwrap();
}

#[test]
fn migration_verifies_current_user_tree_and_preserves_source_and_other_users() {
    let temp = Scratch::new();
    let locator = temp.directory("locator");
    let src = temp.directory("source");
    let dest = temp.directory("destination");
    let mut core = Core::open_at(&locator, Some(&src)).unwrap();
    let user = core.current_user().to_owned();
    let settings = Settings {
        start_at_login: true,
        monitored_roots: vec![temp.directory("observed")],
    };
    core.update_settings(&settings).unwrap();
    let original_cfg = fs::read(src.join(&user).join("core.json")).unwrap();
    fs::write(src.join("shared-note"), b"outside user scope").unwrap();
    fs::create_dir(src.join("other-account")).unwrap();
    fs::write(src.join("other-account/private"), b"other account").unwrap();
    let sample = src.join(&user).join("segments");
    fs::create_dir(&sample).unwrap();
    fs::create_dir(sample.join("empty")).unwrap();
    let bytes: Vec<_> = (0..40_000).map(|i| (i % 251) as u8).collect();
    fs::write(sample.join("one.bin"), &bytes).unwrap();

    core.select_directory(&dest).unwrap();
    assert_eq!(core.selected_root(), Some(dest.as_path()));
    assert_eq!(core.settings().unwrap(), settings);
    drop(core);
    let mut reopened = Core::open_at(&locator, Some(&src)).unwrap();
    assert_eq!(reopened.selected_root(), Some(dest.as_path()));
    assert_eq!(reopened.settings().unwrap(), settings);
    for root in [&src, &dest] {
        let base = root.join(&user);
        assert_eq!(fs::read(base.join("core.json")).unwrap(), original_cfg);
        assert_eq!(fs::read(base.join("segments/one.bin")).unwrap(), bytes);
        assert!(base.join("segments/empty").is_dir());
    }
    assert_eq!(
        fs::read(src.join("other-account/private")).unwrap(),
        b"other account"
    );
    assert!(!dest.join("other-account").exists());
    assert!(!dest.join("shared-note").exists());
}

#[test]
fn rejected_nested_or_nonempty_destination_preserves_the_original_locator() {
    let temp = Scratch::new();
    let locator = temp.directory("locator");
    let src = temp.directory("source");
    let destination = temp.directory("destination");
    let mut core = Core::open_at(&locator, Some(&src)).unwrap();
    let user = core.current_user().to_owned();
    let current = src.join(&user);
    let original = fs::read(current.join("core.json")).unwrap();
    let pointer = saved_location(&locator);
    let nested = current.join("nested");
    fs::create_dir(&nested).unwrap();
    assert!(core.select_directory(&nested).is_err());
    assert_eq!(fs::read_dir(&nested).unwrap().count(), 0);

    let dst = platform::Anchor::open(&destination).unwrap();
    let private = dst.child(OsStr::new(&user), true, true).unwrap();
    fs::write(private.io_path().join("unrelated"), b"preserve").unwrap();
    drop(private);
    drop(dst);
    assert!(core.select_directory(&destination).is_err());
    assert_eq!(saved_location(&locator), pointer);
    assert_eq!(fs::read(current.join("core.json")).unwrap(), original);
    assert_eq!(
        fs::read(destination.join(&user).join("unrelated")).unwrap(),
        b"preserve"
    );
    assert_eq!(core.selected_root(), Some(src.as_path()));
}

#[test]
fn missing_or_replaced_volume_requires_the_expected_marker_or_an_explicit_new_store() {
    let temp = Scratch::new();
    let locator = temp.directory("locator");
    let root = temp.directory("mounted");
    let fallback = temp.directory("fallback");
    let mut core = Core::open_at(&locator, Some(&root)).unwrap();
    let user = core.current_user().to_owned();
    let settings = Settings {
        start_at_login: true,
        monitored_roots: vec![],
    };
    core.update_settings(&settings).unwrap();
    let pointer = saved_location(&locator);
    let offline = temp.0.join("offline");
    fs::rename(&root, &offline).unwrap();
    assert!(core.update_settings(&Settings::default()).is_err());
    drop(core);
    let mut core = Core::open_at(&locator, Some(&fallback)).unwrap();
    needs_directory(&core);
    assert_eq!(saved_location(&locator), pointer);
    assert!(!fallback.join(&user).exists());
    fs::rename(&offline, &root).unwrap();
    core.select_directory(&root).unwrap();
    assert_eq!(core.settings().unwrap(), settings);

    fs::rename(&root, &offline).unwrap();
    fs::create_dir(&root).unwrap();
    let other_locator = temp.directory("other-locator");
    let mut replacement = Core::open_at(&other_locator, Some(&root)).unwrap();
    assert_eq!(replacement.settings().unwrap(), Settings::default());
    drop(replacement);
    let other_config = fs::read(root.join(&user).join("core.json")).unwrap();
    assert!(core.settings().is_err());
    assert!(core.update_settings(&settings).is_err());
    assert_eq!(
        fs::read(root.join(&user).join("core.json")).unwrap(),
        other_config
    );
    assert_eq!(saved_location(&locator), pointer);
    assert!(core.select_directory(&fallback).is_err());
    core.select_empty_directory(&fallback).unwrap();
    assert_eq!(core.selected_root(), Some(fallback.as_path()));
    assert_eq!(core.settings().unwrap(), Settings::default());
    assert!(offline.join(user).join("core.json").is_file());
}
