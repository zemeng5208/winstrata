mod backup;
mod bus;
mod crypto;
mod file_registry;
mod files;
mod hybrid;
mod migration;
mod network;
mod platform;
mod plugins;
mod process;
mod runtime;
mod schedule;
mod storage;
mod vault;

use platform::Anchor;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use storage::{
    Config, Location, POINTER, capacity, data_dir, require_disjoint, require_independent_root,
    validate_path,
};

pub use backup::{BackupKind, BackupReport, RestoreReport};
pub use bus::{BusStats, EventBus, Filter, PublishError, PublishOutcome, Subscription};
pub use file_registry::{
    Channel, ChannelView, CollectError, FILE_PROVIDER, FileRegistry, REGISTRY_PROVIDER,
};
pub use hybrid::{
    Batch, CapacityReport, CaptureWrite, Confidence, DataClass, Hybrid, ObjectRecord, Point,
    RecordRef, Recovery, Relation, RiskTag as HybridRiskTag, Snapshot, SnapshotEntry,
    StoredCapture,
};
pub use network::{
    DiskPolicy, FlowFact, InterfaceFact, PacketClass, PacketLedger, PersistFailure, RiskTag,
    flow_event, interface_event,
};
pub use plugins::{
    DependencyStatus, OFFICIAL, OfficialPlugin, PluginError, PluginHost, PluginReport,
};
pub use process::{
    MODULE_LOADED, MODULE_UNLOADED, PROCESS_EXITED, PROCESS_PROVIDER, PROCESS_SERVICE,
    PROCESS_STARTED, PROCESS_UNRESOLVED, ProcessNode, ProcessTracker, THREAD_EXITED,
    THREAD_STARTED,
};
pub use runtime::{
    Limits, MAX_ADMISSION_WAIT, MAX_QUEUE, MAX_SUBSCRIPTIONS, MAX_TASK_TIMEOUT, MAX_WORKERS,
    Runtime, RuntimeError,
};
pub use schedule::{Cancel, Scheduler, SchedulerStats, Task, TaskId};
pub use storage::Settings;
pub use vault::Protection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Stopped,
    Live,
    History,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageState {
    Ready,
    NeedsDirectory { kind: ErrorKind, detail: String },
}

pub struct Core {
    user: String,
    locator_path: PathBuf,
    session: File,
    location: Option<Location>,
    state: StorageState,
    run: RunState,
    seal: Option<vault::Seal>,
}

impl Drop for Core {
    fn drop(&mut self) {
        // 正常关闭时主动解锁，避免 Linux 并发 fork 到 exec 之间短暂继承同一打开文件描述。
        let _ = self.session.unlock();
    }
}

impl Core {
    pub fn open() -> io::Result<Self> {
        #[cfg(windows)]
        let default = Some(Path::new(r"D:\WinStrata"));
        #[cfg(target_os = "linux")]
        let default = None;
        Self::open_at(&platform::locator_parent()?, default)
    }

    pub fn open_at(parent: &Path, default: Option<&Path>) -> io::Result<Self> {
        let user = platform::current_id()?;
        let locator = Anchor::open(parent)?.child(OsStr::new("WinStrata"), true, true)?;
        let session = files::lock(&locator, "session.lock")?;
        let location = Location::read(&locator)?;
        drop(locator);
        let mut core = Self {
            user,
            locator_path: parent.join("WinStrata"),
            session,
            location,
            state: StorageState::NeedsDirectory {
                kind: ErrorKind::NotFound,
                detail: "select a storage directory".into(),
            },
            run: RunState::Stopped,
            seal: None,
        };
        let candidate = core.selected_root().or(default).map(Path::to_owned);
        if let Some(path) = candidate {
            if let Err(error) = core.select_directory(&path) {
                core.suspend(&error);
            }
        }
        Ok(core)
    }

    pub fn current_user(&self) -> &str {
        &self.user
    }

    pub fn locator_path(&self) -> &Path {
        &self.locator_path
    }

    pub fn selected_root(&self) -> Option<&Path> {
        self.location
            .as_ref()
            .map(|location| location.root.as_path())
    }

    pub fn storage_state(&self) -> &StorageState {
        &self.state
    }
    pub fn run_state(&self) -> RunState {
        self.run
    }

    fn suspend(&mut self, error: &io::Error) {
        self.state = StorageState::NeedsDirectory {
            kind: error.kind(),
            detail: error.to_string(),
        };
        if self.run == RunState::Live {
            self.stop();
        }
    }

    fn active(&mut self, required: u64) -> io::Result<(Anchor, Config)> {
        if let StorageState::NeedsDirectory { kind, detail } = &self.state {
            return Err(io::Error::new(*kind, detail.clone()));
        }
        let result = self.bind().and_then(|(dir, config)| {
            capacity(&dir, required)?;
            Ok((dir, config))
        });
        if let Err(error) = &result {
            self.suspend(error);
        }
        result
    }

    fn locator(&self) -> io::Result<Anchor> {
        let parent = self
            .locator_path
            .parent()
            .expect("locator is beneath the OS state directory");
        let dir = Anchor::open(parent)?.child(OsStr::new("WinStrata"), false, true)?;
        files::verify_session(&dir, &self.session)?;
        Ok(dir)
    }

    fn bind(&self) -> io::Result<(Anchor, Config)> {
        let location = self
            .location
            .as_ref()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "select a storage directory"))?;
        self.locator()?;
        location.bind(&self.user)
    }

    pub fn check_capacity(&mut self, required: u64) -> io::Result<()> {
        self.active(required).map(|_| ())
    }

    pub fn settings(&mut self) -> io::Result<Settings> {
        Ok(self.active(0)?.1.settings)
    }

    pub fn update_settings(&mut self, settings: &Settings) -> io::Result<()> {
        settings.validate()?;
        let (dir, mut config) = self.active(0)?;
        config.settings = settings.clone();
        if let Err(error) = config.save(&dir) {
            if error.kind() != ErrorKind::InvalidInput {
                self.suspend(&error);
            }
            return Err(error);
        }
        if self.run == RunState::Live && settings.monitored_roots.is_empty() {
            self.stop();
        }
        Ok(())
    }

    pub fn start_live(&mut self) -> io::Result<()> {
        let (dir, config) = self.active(0)?;
        if config.settings.monitored_roots.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "select at least one monitoring scope",
            ));
        }
        if let Err(error) = files::probe(&dir) {
            self.suspend(&error);
            return Err(error);
        }
        self.run = RunState::Live;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.run = RunState::Stopped;
    }

    pub fn enter_history(&mut self) {
        self.run = RunState::History;
    }

    // 原子更名后若目录同步失败，下一次选择以磁盘上实际发布的指针为准。
    fn refresh_location(&mut self) -> io::Result<()> {
        let result = self
            .locator()
            .and_then(|dir| Location::read(&dir))
            .and_then(|location| {
                if self.location.is_some() && location.is_none() {
                    return Err(io::Error::new(
                        ErrorKind::NotFound,
                        "the saved storage locator is missing",
                    ));
                }
                Ok(location)
            });
        match result {
            Ok(location) => {
                self.location = location;
                Ok(())
            }
            Err(error) => {
                self.suspend(&error);
                Err(error)
            }
        }
    }

    fn check_destination(&self, root: &Path) -> io::Result<PathBuf> {
        validate_path(root)?;
        let leaf = root.join(&self.user);
        require_disjoint(&leaf, &self.locator_path, &self.locator()?)?;
        Ok(leaf)
    }

    fn publish(&mut self, root: &Path, target: &Anchor, config: &Config) -> io::Result<()> {
        let location = Location::new(root, config);
        let (reachable, _) = location.bind(&self.user)?;
        if !target.same_directory(&reachable)? {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "destination directory changed before publishing the locator",
            ));
        }
        if let Err(error) = self
            .locator()
            .and_then(|dir| files::write_json(&dir, POINTER, &location))
        {
            self.suspend(&error);
            return Err(error);
        }
        self.location = Some(location);
        self.state = StorageState::Ready;
        self.stop();
        self.sync_protection()?;
        Ok(())
    }

    fn sync_protection(&mut self) -> io::Result<()> {
        let Ok((dir, config)) = self.bind() else {
            self.seal = None;
            return Ok(());
        };
        if !vault::exists(&dir)? {
            self.seal = None;
            return Ok(());
        }
        if self.seal.as_ref().is_some_and(|seal| {
            seal.user() != self.user || seal.workspace() != config.workspace_id()
        }) {
            self.seal = None;
        }
        Ok(())
    }

    fn ready_dir(&mut self) -> io::Result<(Anchor, Config)> {
        self.active(0)
    }

    fn is_selected_directory(&self, root: &Path) -> io::Result<bool> {
        let Some(selected) = self.selected_root() else {
            return Ok(false);
        };
        if selected == root {
            return Ok(true);
        }
        // 盘符别名与大小写不同也可能指向当前目录；以实际目录身份判定是否为重选。
        match (
            data_dir(selected, &self.user, false),
            data_dir(root, &self.user, false),
        ) {
            (Ok(current), Ok(candidate)) => current.same_directory(&candidate),
            _ => Ok(false),
        }
    }

    pub fn select_directory(&mut self, root: &Path) -> io::Result<()> {
        self.refresh_location()?;
        let target_path = self.check_destination(root)?;
        if self.is_selected_directory(root)? {
            let result = self.bind().and_then(|(dir, _)| {
                capacity(&dir, 1)?;
                files::probe(&dir)
            });
            return match result {
                Ok(()) => {
                    self.state = StorageState::Ready;
                    self.sync_protection()
                }
                Err(error) => {
                    self.suspend(&error);
                    Err(error)
                }
            };
        }
        if self.run == RunState::Live {
            return Err(io::Error::new(
                ErrorKind::ResourceBusy,
                "stop live monitoring before switching storage",
            ));
        }
        let source = match &self.location {
            Some(location) => {
                let source_path = location.root.join(&self.user);
                let (dir, _) = match self.bind() {
                    Ok(source) => source,
                    Err(error) => {
                        self.suspend(&error);
                        return Err(error);
                    }
                };
                require_disjoint(&target_path, &source_path, &dir)?;
                Some(dir)
            }
            None => None,
        };
        let target = data_dir(root, &self.user, true)?;
        capacity(&target, 1)?;
        files::probe(&target)?;
        let config = if let Some(source) = source {
            migration::copy_and_verify(&source, &target)?;
            let config = Config::load(&target, &self.user)?;
            let (still_selected, original) = self.bind()?;
            if !source.same_directory(&still_selected)? || !config.same_workspace(&original) {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "source workspace changed during migration",
                ));
            }
            config
        } else {
            match Config::load(&target, &self.user) {
                Ok(config) => config,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    Config::initialize(&target, &self.user)?
                }
                Err(error) => return Err(error),
            }
        };
        self.publish(root, &target, &config)
    }

    // 在已选择的用户目录下打开插件宿主。尚未选择存储时拒绝。
    pub fn open_plugins(&mut self, runtime: &Path) -> Result<PluginHost, PluginError> {
        let (dir, _) = self
            .active(0)
            .map_err(|error| PluginError::new("storage_unavailable", error.to_string()))?;
        let plugins = dir
            .child(OsStr::new("plugins"), true, true)
            .map_err(|error| PluginError::new("storage_unavailable", error.to_string()))?;
        PluginHost::open(plugins, runtime)
    }

    // 在当前用户目录打开混合存储。卷错误会暂停会话；记录内容错误不会。
    pub fn open_hybrid(&mut self) -> io::Result<Hybrid> {
        let (dir, config) = self.active(1)?;
        let opened = if vault::exists(&dir)? {
            let Some(seal) = self.seal.clone() else {
                return Err(vault::locked_error());
            };
            if seal.is_locked() {
                return Err(vault::locked_error());
            }
            if seal.user() != self.user || seal.workspace() != config.workspace_id() {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "vault identity does not match this workspace",
                ));
            }
            Hybrid::open_sealed(&dir, seal)
        } else if self.seal.is_some() {
            Err(io::Error::new(
                ErrorKind::InvalidData,
                "encryption state does not match the workspace",
            ))
        } else {
            Hybrid::open(&dir)
        };
        match opened {
            Ok(store) => Ok(store),
            Err(error) => {
                if hybrid::suspends(&error) {
                    self.suspend(&error);
                }
                Err(error)
            }
        }
    }

    // 把一批混合记录写入已打开的存储。卷错误会暂停会话并停止实时模式。
    pub fn commit_hybrid(&mut self, store: &mut Hybrid, batch: Batch) -> io::Result<u64> {
        if let StorageState::NeedsDirectory { kind, detail } = &self.state {
            return Err(io::Error::new(*kind, detail.clone()));
        }
        match store.commit(batch) {
            Ok(commit) => Ok(commit),
            Err(error) => {
                if hybrid::suspends(&error) {
                    self.suspend(&error);
                }
                Err(error)
            }
        }
    }

    // 用户明确选择新的空工作区，原卷离线时仍能开始工作；此操作不代表已迁移历史。
    pub fn select_empty_directory(&mut self, root: &Path) -> io::Result<()> {
        self.refresh_location()?;
        self.check_destination(root)?;
        if let Some(location) = &self.location {
            if location.root == root || self.bind().is_ok() {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    "use verified migration for an available workspace",
                ));
            }
            require_independent_root(root, &location.root)?;
        }
        let target = data_dir(root, &self.user, true)?;
        capacity(&target, 1)?;
        files::probe(&target)?;
        let config = Config::initialize(&target, &self.user)?;
        self.publish(root, &target, &config)
    }

    pub fn protection(&mut self) -> io::Result<Protection> {
        let (dir, _) = self.ready_dir()?;
        if let Some(header) = vault::peek(&dir)? {
            Ok(Protection {
                encrypted: true,
                locked: self.seal.as_ref().is_none_or(|seal| seal.is_locked()),
                auto_lock_secs: header.auto_lock_secs,
            })
        } else {
            Ok(Protection {
                encrypted: false,
                locked: false,
                auto_lock_secs: 0,
            })
        }
    }

    pub fn device_id(&mut self) -> io::Result<[u8; 16]> {
        let (dir, config) = self.ready_dir()?;
        vault::device_id(&dir, &self.user, config.workspace_id())
    }

    pub fn enable_encryption(&mut self, password: &str, auto_lock_secs: u32) -> io::Result<()> {
        self.require_idle()?;
        vault::check_password(password)?;
        vault::check_auto_lock(auto_lock_secs)?;
        let (dir, config) = self.ready_dir()?;
        if vault::exists(&dir)? {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "encryption is already enabled",
            ));
        }
        let store = Hybrid::open(&dir)?;
        if store.commits() != 0 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "enable encryption before the first stored record",
            ));
        }
        drop(store);
        let device = vault::device_id(&dir, &self.user, config.workspace_id())?;
        self.seal = Some(vault::create(
            &dir,
            &config,
            &self.user,
            password,
            auto_lock_secs,
            device,
        )?);
        Ok(())
    }

    pub fn unlock(&mut self, password: &str) -> io::Result<()> {
        let (dir, config) = self.ready_dir()?;
        if !vault::exists(&dir)? {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "encryption is not enabled",
            ));
        }
        let opened = vault::unlock(&dir, &config, &self.user, password)?;
        if let Some(current) = &self.seal {
            if current.user() == opened.user() && current.workspace() == opened.workspace() {
                let mut dek = opened.export_dek()?;
                current.replace_dek(dek);
                crypto::wipe(&mut dek);
                return Ok(());
            }
        }
        self.seal = Some(opened);
        Ok(())
    }

    pub fn lock(&mut self) -> io::Result<()> {
        let (dir, _) = self.ready_dir()?;
        if !vault::exists(&dir)? {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "encryption is not enabled",
            ));
        }
        let Some(seal) = &self.seal else {
            return Err(vault::locked_error());
        };
        seal.lock();
        Ok(())
    }

    pub fn change_password(&mut self, current: &str, next: &str) -> io::Result<()> {
        self.require_idle()?;
        let (dir, config) = self.ready_dir()?;
        let Some(seal) = self.seal.clone() else {
            return Err(vault::locked_error());
        };
        vault::change_password(
            &dir,
            &seal,
            &self.user,
            config.workspace_id(),
            current,
            next,
        )
    }

    // 轮换会先重写分段，再重新封装保险库。两步之间进程崩溃时，新密文可能还对应旧封装。
    // 重新封装失败时，本进程会尝试把分段写回旧密钥；那次回写再失败就必须停下来核对备份。
    pub fn rotate_keys(&mut self, store: &mut Hybrid, password: &str) -> io::Result<()> {
        self.require_idle()?;
        let (dir, config) = self.ready_dir()?;
        let Some(seal) = self.seal.clone() else {
            return Err(vault::locked_error());
        };
        if !store.same_seal(&seal) {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "hybrid store does not belong to this session",
            ));
        }
        let mut next = [0u8; 32];
        crypto::random_bytes(&mut next)?;
        let mut previous = match seal.export_dek() {
            Ok(key) => key,
            Err(error) => {
                crypto::wipe(&mut next);
                return Err(error);
            }
        };
        let device = vault::device_id(&dir, &self.user, config.workspace_id())?;
        let seconds = seal.auto_lock_secs();
        if let Err(error) = store.rotate_dek(next) {
            crypto::wipe(&mut next);
            crypto::wipe(&mut previous);
            return Err(error);
        }
        if let Err(error) = vault::rewrap(
            &dir,
            &self.user,
            config.workspace_id(),
            password,
            &next,
            seconds,
            device,
        ) {
            crypto::wipe(&mut next);
            let rolled = store.rotate_dek(previous);
            crypto::wipe(&mut previous);
            return Err(rolled.err().unwrap_or(error));
        }
        crypto::wipe(&mut next);
        crypto::wipe(&mut previous);
        Ok(())
    }

    pub fn backup_full(&mut self, package: &Path) -> io::Result<BackupReport> {
        let identity = self.backup_identity()?;
        backup::write_full(&identity.dir, package, &identity.body)
    }

    pub fn backup_incremental(&mut self, package: &Path, base: &Path) -> io::Result<BackupReport> {
        let identity = self.backup_identity()?;
        backup::write_incremental(&identity.dir, package, base, &identity.body)
    }

    pub fn verify_backup(&self, package: &Path) -> io::Result<BackupReport> {
        let _ = self;
        backup::verify(package)
    }

    pub fn restore_full(&mut self, root: &Path, package: &Path) -> io::Result<RestoreReport> {
        self.require_idle()?;
        let report = backup::restore_full(root, &self.user, package)?;
        self.seal = None;
        Ok(report)
    }

    pub fn restore_incremental(
        &mut self,
        root: &Path,
        base: &Path,
        package: &Path,
    ) -> io::Result<RestoreReport> {
        self.require_idle()?;
        if self.selected_root() != Some(root) {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "incremental restore continues the selected workspace",
            ));
        }
        let report = backup::restore_incremental(root, &self.user, base, package)?;
        self.seal = None;
        Ok(report)
    }

    fn require_idle(&self) -> io::Result<()> {
        if self.run == RunState::Live {
            Err(io::Error::new(
                ErrorKind::ResourceBusy,
                "stop monitoring before changing storage protection",
            ))
        } else {
            Ok(())
        }
    }

    fn backup_identity(&mut self) -> io::Result<BackupIdentity> {
        self.require_idle()?;
        let (dir, config) = self.ready_dir()?;
        let encrypted = vault::exists(&dir)?;
        let device_id = vault::device_id(&dir, &self.user, config.workspace_id())?;
        Ok(BackupIdentity {
            dir,
            body: backup::Identity {
                user: self.user.clone(),
                workspace: config.workspace_id().to_owned(),
                device_id,
                encrypted,
            },
        })
    }
}

struct BackupIdentity {
    dir: Anchor,
    body: backup::Identity,
}

#[cfg(test)]
mod protect_tests;

#[cfg(test)]
mod recovery_tests;

#[cfg(test)]
mod runtime_tests;

#[cfg(test)]
mod tests;
