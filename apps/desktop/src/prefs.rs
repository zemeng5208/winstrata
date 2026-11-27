use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
    Zh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CloseChoice {
    Ask,
    Tray,
    Exit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    theme: Option<Theme>,
    language: Language,
    close: CloseChoice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub theme: Option<Theme>,
    pub language: Language,
    pub close: CloseChoice,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: None,
            language: Language::En,
            close: CloseChoice::Ask,
        }
    }
}

impl Prefs {
    pub fn load(path: &Path) -> io::Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        if bytes.len() > 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "gui preferences exceed 64 KiB",
            ));
        }
        let stored: Stored = serde_json::from_slice(&bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "gui preferences are invalid")
        })?;
        if stored.version != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "gui preferences version is not supported",
            ));
        }
        Ok(Self {
            theme: stored.theme,
            language: stored.language,
            close: stored.close,
        })
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let stored = Stored {
            version: 1,
            theme: self.theme,
            language: self.language,
            close: self.close,
        };
        let bytes = serde_json::to_vec_pretty(&stored)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        let temp = temp_path(path);
        {
            let mut file = create_private(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        replace(&temp, path)?;
        Ok(())
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(unix)]
fn create_private(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let _ = fs::remove_file(path);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> io::Result<File> {
    let _ = fs::remove_file(path);
    OpenOptions::new().write(true).create_new(true).open(path)
}

#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let mut source: Vec<u16> = from.as_os_str().encode_wide().collect();
    let mut destination: Vec<u16> = to.as_os_str().encode_wide().collect();
    source.push(0);
    destination.push(0);
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}
