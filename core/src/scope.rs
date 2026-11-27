//! 用户选择的采集范围。
//!
//! 未选中的路径、类型、进程、软件、注册表和接口不进入记录。
//! 其他用户的私有配置目录即使落在较宽的根目录下也排除。
//! 判断结果不回显被拒绝的路径。

use std::ffi::OsStr;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::storage;

const MAX_ITEMS: usize = 64;
const MAX_TOKEN: usize = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathClass {
    File,
    Directory,
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionScope {
    pub roots: Vec<PathBuf>,
    #[serde(default)]
    pub file_types: Vec<String>,
    #[serde(default)]
    pub processes: Vec<String>,
    #[serde(default)]
    pub software: Vec<String>,
    #[serde(default)]
    pub registry: Vec<String>,
    #[serde(default)]
    pub interfaces: Vec<String>,
    #[serde(default)]
    pub watch_processes: bool,
}

impl CollectionScope {
    pub fn validate(&self) -> io::Result<()> {
        if self.roots.len() > MAX_ITEMS
            || self.file_types.len() > MAX_ITEMS
            || self.processes.len() > MAX_ITEMS
            || self.software.len() > MAX_ITEMS
            || self.registry.len() > MAX_ITEMS
            || self.interfaces.len() > MAX_ITEMS
        {
            return Err(invalid_input("collection scope exceeds 64 entries"));
        }
        for root in &self.roots {
            storage::validate_path(root)?;
        }
        for token in &self.file_types {
            if !is_extension(token) {
                return Err(invalid_input("file type is not a short extension"));
            }
        }
        for token in self
            .processes
            .iter()
            .chain(&self.software)
            .chain(&self.interfaces)
        {
            if !is_token(token) {
                return Err(invalid_input("scope name is empty or contains controls"));
            }
        }
        for key in &self.registry {
            if !is_registry_key(key) {
                return Err(invalid_input("registry scope is not a relative key"));
            }
        }
        Ok(())
    }

    pub fn watches_processes(&self) -> bool {
        self.watch_processes || !self.processes.is_empty()
    }

    pub fn admits_directory(&self, path: &Path) -> bool {
        self.decide_path(path, PathClass::Directory)
    }

    pub fn admits_file(&self, path: &Path) -> bool {
        self.decide_path(path, PathClass::File)
    }

    pub fn decide_path(&self, path: &Path, class: PathClass) -> bool {
        if path.components().any(|part| part == Component::ParentDir) || !path.is_absolute() {
            return false;
        }
        if foreign_private(path, current_profile().as_deref()) {
            return false;
        }
        if !self.roots.iter().any(|root| under_root(path, root)) {
            return false;
        }
        if class == PathClass::Directory || self.file_types.is_empty() {
            return true;
        }
        match extension_of(path) {
            Some(extension) => self
                .file_types
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(&extension)),
            None => false,
        }
    }

    pub fn admits_process(&self, name: &str, image: Option<&str>) -> bool {
        if !self.watches_processes() {
            return false;
        }
        if !self.processes.is_empty()
            && !self
                .processes
                .iter()
                .any(|item| item.eq_ignore_ascii_case(name))
        {
            return false;
        }
        // 进程镜像不套用文件类型过滤。没有路径就不能证明它落在选定根目录内。
        let Some(image) = image else {
            return false;
        };
        let path = Path::new(image);
        !foreign_private(path, current_profile().as_deref())
            && path.is_absolute()
            && self.roots.iter().any(|root| under_root(path, root))
    }

    pub fn admits_software(&self, name: &str) -> bool {
        self.software
            .iter()
            .any(|item| item.eq_ignore_ascii_case(name))
    }

    pub fn admits_interface(&self, name: &str) -> bool {
        self.interfaces
            .iter()
            .any(|item| item.eq_ignore_ascii_case(name))
    }

    /// 注册表范围只接受当前用户下的相对子键。其他用户的 SID 即使被写入列表也排除。
    pub fn admits_registry(&self, key: &str, current_user: Option<&str>) -> bool {
        if !is_registry_key(key) {
            return false;
        }
        if key_mentions_other_user(key, current_user) {
            return false;
        }
        self.registry
            .iter()
            .any(|selected| key.eq_ignore_ascii_case(selected) || key_under(key, selected))
    }
}

pub fn foreign_private(path: &Path, profile: Option<&Path>) -> bool {
    let Some(owner) = profile_owner(path) else {
        return false;
    };
    if is_public_profile(&owner) {
        return false;
    }
    match profile.and_then(profile_owner) {
        Some(ours) => !owner.eq_ignore_ascii_case(&ours),
        None => true,
    }
}

fn current_profile() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let value = std::env::var_os(key)?;
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn profile_owner(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let parts: Vec<&str> = text
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    let mut index = 0;
    if parts.first().is_some_and(|part| part.ends_with(':')) {
        index = 1;
    }
    let kind = *parts.get(index)?;
    if kind.eq_ignore_ascii_case("home")
        || kind.eq_ignore_ascii_case("Users")
        || kind.eq_ignore_ascii_case("Documents and Settings")
    {
        return parts.get(index + 1).map(|name| (*name).to_owned());
    }
    if index == 0 && kind.eq_ignore_ascii_case("root") {
        return Some("root".to_owned());
    }
    None
}

fn is_public_profile(name: &str) -> bool {
    ["Public", "Default", "Default User", "All Users"]
        .iter()
        .any(|item| name.eq_ignore_ascii_case(item))
}

fn under_root(path: &Path, root: &Path) -> bool {
    let mut path_parts = path.components();
    for part in root.components() {
        match path_parts.next() {
            Some(next) if component_eq(next.as_os_str(), part.as_os_str()) => {}
            _ => return false,
        }
    }
    true
}

fn component_eq(left: &OsStr, right: &OsStr) -> bool {
    if left == right {
        return true;
    }
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn extension_of(path: &Path) -> Option<String> {
    let text = path.file_name()?.to_str()?;
    let (_, extension) = text.rsplit_once('.')?;
    if extension.is_empty() || extension.eq_ignore_ascii_case(text) {
        None
    } else {
        Some(extension.to_ascii_lowercase())
    }
}

fn key_under(key: &str, selected: &str) -> bool {
    key.len() > selected.len()
        && key.as_bytes().get(selected.len()) == Some(&b'\\')
        && key[..selected.len()].eq_ignore_ascii_case(selected)
}

fn key_mentions_other_user(key: &str, current_user: Option<&str>) -> bool {
    let lower = key.to_ascii_lowercase();
    if !lower.contains("s-1-") {
        return false;
    }
    match current_user {
        Some(user) if lower.contains(&user.to_ascii_lowercase()) => false,
        _ => true,
    }
}

fn is_extension(token: &str) -> bool {
    (1..=16).contains(&token.len())
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
}

fn is_token(token: &str) -> bool {
    (1..=MAX_TOKEN).contains(&token.len())
        && !token.chars().any(|symbol| symbol.is_control() || symbol == '/' || symbol == '\\')
}

fn is_registry_key(key: &str) -> bool {
    if key.is_empty() || key.len() > 200 || key.starts_with('\\') || key.ends_with('\\') {
        return false;
    }
    let mut count = 0;
    for part in key.split('\\') {
        count += 1;
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.len() > 255
            || part
                .chars()
                .any(|symbol| symbol.is_control() || symbol == '/' || symbol == ':')
            || part.to_ascii_lowercase().starts_with("hklm")
            || part.to_ascii_lowercase().starts_with("hku")
        {
            return false;
        }
    }
    (1..=8).contains(&count)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(root: &Path) -> CollectionScope {
        CollectionScope {
            roots: vec![root.to_path_buf()],
            file_types: vec!["txt".into()],
            ..CollectionScope::default()
        }
    }

    #[test]
    fn foreign_profiles_and_unselected_types_stay_out() {
        let profile = Path::new("/home/me");
        assert!(foreign_private(Path::new("/home/other/secret"), Some(profile)));
        assert!(!foreign_private(Path::new("/home/me/notes"), Some(profile)));
        assert!(!foreign_private(Path::new("/home/Public/share"), Some(profile)));
        assert!(!foreign_private(Path::new("/tmp/notes"), Some(profile)));
        assert!(foreign_private(Path::new("/root/secret"), Some(profile)));
        assert!(foreign_private(
            Path::new(r"C:\Users\other\secret"),
            Some(Path::new(r"C:\Users\me"))
        ));
        assert!(!foreign_private(
            Path::new(r"C:\Users\me\notes"),
            Some(Path::new(r"C:\Users\me"))
        ));
        assert!(!foreign_private(
            Path::new(r"C:\Users\Public\share"),
            Some(Path::new(r"C:\Users\me"))
        ));

        let root = std::env::temp_dir().join("winstrata-scope-chosen");
        let selected = scope(&root);
        assert!(selected.admits_file(&root.join("keep.txt")));
        assert!(!selected.admits_file(&root.join("skip.exe")));
        assert!(!selected.admits_file(&std::env::temp_dir().join("winstrata-scope-other").join("keep.txt")));
        assert!(selected.admits_directory(&root.join("nested")));
        assert!(!selected.decide_path(&root.join("..").join("secret.txt"), PathClass::File));
        let shown = format!(
            "{:?}",
            selected.decide_path(&std::env::temp_dir().join("secret.txt"), PathClass::File)
        );
        assert!(!shown.contains("secret"));
    }

    #[test]
    fn registry_process_and_interface_choices_are_explicit() {
        let root = std::env::temp_dir().join("winstrata-scope-chosen");
        let mut selected = scope(&root);
        selected.registry = vec!["Software\\WinStrata".into()];
        selected.interfaces = vec!["Ethernet".into()];
        selected.software = vec!["Sample".into()];
        selected.processes = vec!["sample".into()];
        assert!(selected.admits_registry(r"Software\WinStrata\State", Some("uid-1")));
        assert!(!selected.admits_registry(r"Software\Other", Some("uid-1")));
        assert!(!selected.admits_registry(r"Software\WinStrata\S-1-5-99", Some("uid-1")));
        assert!(!selected.validate().is_err());
        assert!(!selected.admits_interface("Wi-Fi"));
        assert!(selected.admits_interface("ethernet"));
        assert!(selected.admits_software("sample"));
        assert!(!selected.admits_software("other"));
        let inside = root.join("sample.txt");
        let outside = std::env::temp_dir().join("winstrata-scope-other").join("sample.txt");
        assert!(!selected.admits_process("other", inside.to_str()));
        assert!(selected.admits_process("sample", inside.to_str()));
        assert!(!selected.admits_process("sample", outside.to_str()));
        selected.processes.clear();
        selected.watch_processes = false;
        assert!(!selected.admits_process("sample", inside.to_str()));
    }
}
