use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OFFICIAL_GUI: &str = "winstrata.gui";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    WhiteAcrylic,
    BlackAcrylic,
    LiquidGlass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl Language {
    pub const fn fallback(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosePreference {
    Ask,
    Tray,
    Exit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    theme: Option<Theme>,
    language: Option<Language>,
    close: Option<ClosePreference>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub theme: Option<Theme>,
    pub language: Option<Language>,
    pub close: Option<ClosePreference>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: None,
            language: None,
            close: None,
        }
    }
}

impl Preferences {
    pub fn load(directory: &Path) -> io::Result<Self> {
        let path = directory.join("gui.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(&path)?;
        if bytes.len() > 4096 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "gui preference is too large"));
        }
        let stored: Stored = serde_json::from_slice(&bytes).map_err(|error| {
            io::Error::new(io::ErrorKind::InvalidData, error.to_string())
        })?;
        if stored.version != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "gui preference version is not 1",
            ));
        }
        Ok(Self {
            theme: stored.theme,
            language: stored.language,
            close: stored.close,
        })
    }

    pub fn save(&self, directory: &Path) -> io::Result<()> {
        fs::create_dir_all(directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(directory, fs::Permissions::from_mode(0o700));
        }
        let stored = Stored {
            version: 1,
            theme: self.theme,
            language: self.language,
            close: self.close,
        };
        let bytes = serde_json::to_vec_pretty(&stored)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let temporary = directory.join("gui.json.tmp");
        {
            let mut file = create_private(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(&temporary, directory.join("gui.json"))?;
        Ok(())
    }
}

pub fn preference_directory() -> io::Result<PathBuf> {
    Ok(state_directory()?.join("WinStrata"))
}

fn state_directory() -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        if let Some(value) = std::env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(value));
        }
        let home = std::env::var_os("HOME").filter(|value| !value.is_empty()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "HOME or XDG_STATE_HOME is required")
        })?;
        Ok(PathBuf::from(home).join(".local/state"))
    }
    #[cfg(windows)]
    {
        windows_state_directory()
    }
}

#[cfg(windows)]
fn windows_state_directory() -> io::Result<PathBuf> {
    use std::ptr::null_mut;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath};

    let mut raw = null_mut();
    let result = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, null_mut(), &mut raw) };
    let path = if result < 0 || raw.is_null() {
        Err(io::Error::other(format!("LocalAppData lookup failed ({result:#x})")))
    } else {
        use std::os::windows::ffi::OsStringExt;
        let mut len = 0usize;
        unsafe {
            while *raw.add(len) != 0 {
                len += 1;
            }
            Ok(PathBuf::from(std::ffi::OsString::from_wide(
                std::slice::from_raw_parts(raw, len),
            )))
        }
    };
    unsafe { CoTaskMemFree(raw.cast()) };
    path
}

fn create_private(path: &Path) -> io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// 官方默认界面保持安装。调用它不会删除文件。
pub fn uninstall_official_gui() -> Result<(), &'static str> {
    Err("builtin_retained")
}
