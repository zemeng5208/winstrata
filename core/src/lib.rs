mod files;
mod migration;
mod platform;
mod storage;

use platform::Anchor;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use storage::{
    Config, Location, POINTER, capacity, data_dir, require_disjoint, require_independent_root,
    validate_path,
};

pub use storage::Settings;

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
    locator: Anchor,
    _session: File,
    location: Option<Location>,
    state: StorageState,
    run: RunState,
}

impl Core {
    pub fn open() -> io::Result<Self> {
        #[cfg(windows)]
        let default = Some(Path::new(r"D:\WinStrata"));
        #[cfg(target_os = "linux")]
        let default = None;
        Self::open_at(&platform::locator_parent()?, default)
    }

    fn open_at(parent: &Path, default: Option<&Path>) -> io::Result<Self> {
        let user = platform::current_id()?;
        let locator = Anchor::open(parent)?.child(OsStr::new("WinStrata"), true, true)?;
        let session = files::lock(&locator, "session.lock")?;
        let location = Location::read(&locator)?;
        let mut core = Self {
            user,
            locator_path: parent.join("WinStrata"),
            locator,
            _session: session,
            location,
            state: StorageState::NeedsDirectory {
                kind: ErrorKind::NotFound,
                detail: "select a storage directory".into(),
            },
            run: RunState::Stopped,
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

    fn bind(&self) -> io::Result<(Anchor, Config)> {
        let location = self
            .location
            .as_ref()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "select a storage directory"))?;
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
        let result = Location::read(&self.locator).and_then(|location| {
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
        require_disjoint(&leaf, &self.locator_path, &self.locator)?;
        Ok(leaf)
    }

    fn publish(&mut self, root: &Path, config: &Config) -> io::Result<()> {
        let location = Location::new(root, config);
        if let Err(error) = files::write_json(&self.locator, POINTER, &location) {
            self.suspend(&error);
            return Err(error);
        }
        self.location = Some(location);
        self.state = StorageState::Ready;
        self.stop();
        Ok(())
    }

    pub fn select_directory(&mut self, root: &Path) -> io::Result<()> {
        self.refresh_location()?;
        let target_path = self.check_destination(root)?;
        if self.selected_root() == Some(root) {
            let result = self.bind().and_then(|(dir, _)| {
                capacity(&dir, 1)?;
                files::probe(&dir)
            });
            match &result {
                Ok(_) => self.state = StorageState::Ready,
                Err(error) => self.suspend(error),
            }
            return result;
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
            Config::load(&target, &self.user)?
        } else {
            match Config::load(&target, &self.user) {
                Ok(config) => config,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    Config::initialize(&target, &self.user)?
                }
                Err(error) => return Err(error),
            }
        };
        self.publish(root, &config)
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
        self.publish(root, &config)
    }
}

#[cfg(test)]
mod tests;
