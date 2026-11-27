use crate::{Core, RunState, Settings, StorageState, platform};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "winstrata-recovery-{}",
            platform::fresh_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("locator")).unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn missing_default(root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        // SAFETY: 只读取系统盘符位图。即使随后挂载了驱动器，也只尝试打开唯一且不存在的父目录。
        let drives = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        assert_ne!(drives, 0, "{}", std::io::Error::last_os_error());
        if let Some(index) = (0..26).rev().find(|index| drives & (1u32 << index) == 0) {
            let drive = char::from(b'A' + index as u8);
            let parent = format!("winstrata-recovery-{}", platform::fresh_id().unwrap());
            return PathBuf::from(format!(r"{drive}:\{parent}\WinStrata"));
        }
    }
    root.join("missing-volume/WinStrata")
}

#[test]
fn missing_default_parent_leaves_storage_unselected_until_manual_choice() {
    let fixture = Fixture::new();
    let locator = fixture.0.join("locator");
    let unavailable = missing_default(&fixture.0);
    let manual = fixture.0.join("manual");
    let mut core = Core::open_at(&locator, Some(&unavailable)).unwrap();
    assert!(matches!(
        core.storage_state(),
        StorageState::NeedsDirectory { .. }
    ));
    assert_eq!(core.selected_root(), None);
    assert_eq!(core.run_state(), RunState::Stopped);
    assert!(core.settings().is_err());
    assert!(core.start_live().is_err());
    assert!(!locator.join("WinStrata/location.json").exists());
    assert!(!manual.exists());

    core.select_directory(&manual).unwrap();
    let user = core.current_user().to_owned();
    assert!(manual.join(user).join("core.json").is_file());
    drop(core);
    let reopened = Core::open_at(&locator, Some(&unavailable)).unwrap();
    assert_eq!(reopened.selected_root(), Some(manual.as_path()));
    assert_eq!(reopened.storage_state(), &StorageState::Ready);
}

fn invalid_documents(valid: &[u8]) -> [Vec<u8>; 2] {
    let mut future: serde_json::Value = serde_json::from_slice(valid).unwrap();
    future["version"] = serde_json::json!(2);
    [
        serde_json::to_vec(&future).unwrap(),
        br#"{"version":"#.to_vec(),
    ]
}

#[test]
fn damaged_and_future_documents_leave_saved_bytes_and_storage_choice_intact() {
    let fixture = Fixture::new();
    let locator = fixture.0.join("locator");
    let data = fixture.0.join("data");
    let other = fixture.0.join("another-volume");
    let core = Core::open_at(&locator, Some(&data)).unwrap();
    let config = data.join(core.current_user()).join("core.json");
    let pointer = locator.join("WinStrata/location.json");
    drop(core);
    let original_pointer = fs::read(&pointer).unwrap();
    let original_config = fs::read(&config).unwrap();

    for bad in invalid_documents(&original_pointer) {
        fs::write(&pointer, &bad).unwrap();
        assert!(Core::open_at(&locator, Some(&other)).is_err());
        assert_eq!(fs::read(&pointer).unwrap(), bad);
        assert_eq!(fs::read(&config).unwrap(), original_config);
        assert!(!other.exists());
    }
    fs::write(&pointer, &original_pointer).unwrap();
    for bad in invalid_documents(&original_config) {
        fs::write(&config, &bad).unwrap();
        let mut session = Core::open_at(&locator, Some(&other)).unwrap();
        assert!(matches!(
            session.storage_state(),
            StorageState::NeedsDirectory { .. }
        ));
        assert_eq!(session.selected_root(), Some(data.as_path()));
        assert!(session.settings().is_err());
        assert!(session.update_settings(&Settings::default()).is_err());
        assert!(session.select_directory(&other).is_err());
        assert_eq!(fs::read(&config).unwrap(), bad);
        assert_eq!(fs::read(&pointer).unwrap(), original_pointer);
        assert!(!other.exists());
    }
    fs::write(&config, original_config).unwrap();
    assert_eq!(
        Core::open_at(&locator, Some(&other))
            .unwrap()
            .settings()
            .unwrap(),
        Settings::default()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn public_open_uses_the_child_process_xdg_state_directory() {
    const FIXTURE: &str = "WINSTRATA_TEST_XDG_STORAGE_ROOT";
    if let Some(root) = std::env::var_os(FIXTURE) {
        let data = PathBuf::from(root).join("public-data");
        let mut core = Core::open().unwrap();
        assert_eq!(core.selected_root(), None);
        assert!(matches!(
            core.storage_state(),
            StorageState::NeedsDirectory { .. }
        ));
        core.select_directory(&data).unwrap();
        drop(core);
        assert_eq!(Core::open().unwrap().selected_root(), Some(data.as_path()));
        return;
    }
    let fixture = Fixture::new();
    let state = fixture.0.join("new/xdg-state");
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "recovery_tests::public_open_uses_the_child_process_xdg_state_directory",
        ])
        .env(FIXTURE, &fixture.0)
        .env("XDG_STATE_HOME", &state)
        .env_remove("HOME")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(state.join("WinStrata/location.json").is_file());
    let user = platform::current_id().unwrap();
    assert!(
        fixture
            .0
            .join("public-data")
            .join(user)
            .join("core.json")
            .is_file()
    );
}
