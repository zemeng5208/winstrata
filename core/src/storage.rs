use crate::files;
use crate::platform::{self, Anchor};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

pub(crate) const POINTER: &str = "location.json";
pub(crate) const CONFIG: &str = "core.json";
const VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub start_at_login: bool,
    pub monitored_roots: Vec<PathBuf>,
}

impl Settings {
    pub(crate) fn validate(&self) -> io::Result<()> {
        for path in &self.monitored_roots {
            validate_path(path)?;
        }
        Ok(())
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

pub(crate) fn validate_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "expected an absolute path without parent traversal",
        ));
    }
    #[cfg(windows)]
    if !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_)))
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "expected a local drive path",
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Location {
    version: u32,
    pub(crate) root: PathBuf,
    workspace_id: String,
}

impl Location {
    pub(crate) fn read(dir: &Anchor) -> io::Result<Option<Self>> {
        match files::read_json::<Self>(dir, POINTER) {
            Ok(location) => {
                location.validate()?;
                Ok(Some(location))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn new(root: &Path, config: &Config) -> Self {
        Self {
            version: VERSION,
            root: root.into(),
            workspace_id: config.workspace_id.clone(),
        }
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        if self.version != VERSION || !valid_id(&self.workspace_id) {
            return Err(invalid("unsupported or invalid storage locator"));
        }
        validate_path(&self.root)
    }

    pub(crate) fn bind(&self, user: &str) -> io::Result<(Anchor, Config)> {
        let dir = data_dir(&self.root, user, false)?;
        let config = Config::load(&dir, user)?;
        if config.workspace_id != self.workspace_id {
            return Err(invalid(
                "the selected directory contains a different workspace",
            ));
        }
        Ok((dir, config))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    version: u32,
    owner: String,
    workspace_id: String,
    pub(crate) settings: Settings,
}

fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}

impl Config {
    pub(crate) fn initialize(dir: &Anchor, user: &str) -> io::Result<Self> {
        if fs::read_dir(dir.io_path())?.next().transpose()?.is_some() {
            return Err(io::Error::new(
                ErrorKind::DirectoryNotEmpty,
                "new workspace requires an empty private directory",
            ));
        }
        let config = Self {
            version: VERSION,
            owner: user.into(),
            workspace_id: platform::fresh_id()?,
            settings: Settings::default(),
        };
        config.save(dir)?;
        Ok(config)
    }

    pub(crate) fn load(dir: &Anchor, user: &str) -> io::Result<Self> {
        let config: Self = files::read_json(dir, CONFIG)?;
        if config.version != VERSION || config.owner != user || !valid_id(&config.workspace_id) {
            return Err(invalid(
                "unsupported configuration, owner or workspace identifier",
            ));
        }
        config.settings.validate()?;
        Ok(config)
    }

    pub(crate) fn save(&self, dir: &Anchor) -> io::Result<()> {
        let bytes = files::encode_json(self)?;
        capacity(dir, bytes.len() as u64)?;
        files::write_encoded(dir, CONFIG, &bytes)
    }
}

pub(crate) fn capacity(dir: &Anchor, required: u64) -> io::Result<()> {
    if dir.available()? < required.max(1) {
        return Err(io::Error::new(
            ErrorKind::StorageFull,
            "insufficient space available to the current user",
        ));
    }
    Ok(())
}

pub(crate) fn data_dir(root: &Path, user: &str, create: bool) -> io::Result<Anchor> {
    validate_path(root)?;
    let root = match (root.parent(), root.file_name()) {
        (Some(parent), Some(leaf)) => Anchor::open(parent)?.child(leaf, create, false)?,
        _ => Anchor::open(root)?,
    };
    root.child(OsStr::new(user), create, true)
}

// 在创建目录前比较已有祖先的内核身份，处理卷别名与大小写差异。
fn contains(path: &Path, needle: &Anchor) -> io::Result<bool> {
    for ancestor in path.ancestors() {
        match Anchor::open(ancestor) {
            Ok(dir) if dir.same_directory(needle)? => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

pub(crate) fn require_disjoint(
    candidate: &Path,
    protected_path: &Path,
    protected: &Anchor,
) -> io::Result<()> {
    if contains(candidate, protected)? {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "storage directories overlap",
        ));
    }
    match Anchor::open(candidate) {
        Ok(dir) if contains(protected_path, &dir)? => Err(io::Error::new(
            ErrorKind::InvalidInput,
            "storage directories overlap",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn require_independent_root(candidate: &Path, previous: &Path) -> io::Result<()> {
    // 离线卷无法返回文件身份，仍需按本机路径规则拒绝原根与其上下级。
    let mut common = true;
    for (a, b) in candidate.components().zip(previous.components()) {
        if !component_eq(a.as_os_str(), b.as_os_str())? {
            common = false;
            break;
        }
    }
    if common {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "select an independent storage root",
        ));
    }
    if let Ok(existing) = Anchor::open(previous) {
        require_disjoint(candidate, previous, &existing)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn component_eq(a: &OsStr, b: &OsStr) -> io::Result<bool> {
    Ok(a == b)
}

#[cfg(windows)]
fn component_eq(a: &OsStr, b: &OsStr) -> io::Result<bool> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Globalization::{CSTR_EQUAL, CompareStringOrdinal};
    let a: Vec<u16> = a.encode_wide().collect();
    let b: Vec<u16> = b.encode_wide().collect();
    // Windows 的序数忽略大小写比较不会受当前语言区域或显示格式影响。
    let result =
        unsafe { CompareStringOrdinal(a.as_ptr(), a.len() as i32, b.as_ptr(), b.len() as i32, 1) };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result == CSTR_EQUAL)
    }
}
