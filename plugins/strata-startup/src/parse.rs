//! 启动项文本的只读解析。
//!
//! 这里不执行命令，也不改文件。快捷方式只取 Shell Link 文档里的本地路径。

#[cfg(any(target_os = "linux", test))]
use std::fmt::Write as _;

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopFile {
    pub exec: String,
    pub enabled: bool,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CronEntry {
    pub line: usize,
    pub schedule: String,
    pub user: String,
    pub command: String,
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_desktop(text: &str) -> DesktopFile {
    let mut in_entry = false;
    let mut exec = String::new();
    let mut hidden = false;
    let mut gnome_disabled = false;
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "Exec" && exec.is_empty() {
            exec = value.to_owned();
        } else if key == "Hidden" && value.eq_ignore_ascii_case("true") {
            hidden = true;
        } else if key == "X-GNOME-Autostart-enabled" && value.eq_ignore_ascii_case("false") {
            gnome_disabled = true;
        }
    }
    DesktopFile {
        exec,
        enabled: !hidden && !gnome_disabled,
    }
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_crontab(text: &str, system: bool) -> Vec<CronEntry> {
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some(first) = tokens.first() else {
            continue;
        };
        if first.contains('=') && !first.starts_with('@') {
            continue;
        }
        let parsed = if first.starts_with('@') {
            if system {
                if tokens.len() < 3 {
                    continue;
                }
                Some((
                    (*first).to_owned(),
                    tokens[1].to_owned(),
                    tokens[2..].join(" "),
                ))
            } else if tokens.len() < 2 {
                continue;
            } else {
                Some(((*first).to_owned(), String::new(), tokens[1..].join(" ")))
            }
        } else if system {
            if tokens.len() < 7 {
                continue;
            }
            Some((
                tokens[..5].join(" "),
                tokens[5].to_owned(),
                tokens[6..].join(" "),
            ))
        } else if tokens.len() < 6 {
            continue;
        } else {
            Some((tokens[..5].join(" "), String::new(), tokens[5..].join(" ")))
        };
        let Some((schedule, user, command)) = parsed else {
            continue;
        };
        if command.is_empty() {
            continue;
        }
        entries.push(CronEntry {
            line: index + 1,
            schedule,
            user,
            command,
        });
    }
    entries
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_timer(text: &str) -> (String, String) {
    let mut schedule = String::new();
    let mut unit = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if matches!(
            key,
            "OnCalendar"
                | "OnBootSec"
                | "OnStartupSec"
                | "OnUnitActiveSec"
                | "OnUnitInactiveSec"
                | "OnActiveSec"
        ) {
            if !schedule.is_empty() {
                schedule.push('|');
            }
            let _ = write!(schedule, "{key}={value}");
        } else if key == "Unit" && unit.is_empty() {
            unit = value.to_owned();
        }
    }
    (schedule, unit)
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_exec_start(text: &str) -> String {
    let mut in_service = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_service = line == "[Service]";
            continue;
        }
        if !in_service {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "ExecStart" {
            return value.trim().to_owned();
        }
    }
    String::new()
}

/// 读取 [MS-SHLLINK] LinkInfo 里的本地路径。解析不了就返回 None，不猜目标。
#[cfg_attr(all(not(windows), not(test)), allow(dead_code))]
pub(crate) fn link_local_path(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 76 {
        return None;
    }
    let header_size = u32_at(bytes, 0)? as usize;
    if header_size != 0x4c || bytes.len() < header_size {
        return None;
    }
    let flags = u32_at(bytes, 20)?;
    let mut cursor = header_size;
    if flags & 0x1 != 0 {
        if bytes.len() < cursor + 2 {
            return None;
        }
        let list = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().ok()?) as usize;
        cursor = cursor.checked_add(2)?.checked_add(list)?;
    }
    if flags & 0x2 == 0 || bytes.len() < cursor.checked_add(28)? {
        return None;
    }
    let info = &bytes[cursor..];
    let info_size = u32_at(info, 0)? as usize;
    if info_size < 28 || info.len() < info_size {
        return None;
    }
    let header = u32_at(info, 4)? as usize;
    let info_flags = u32_at(info, 8)?;
    if info_flags & 0x1 == 0 {
        return None;
    }
    let local_off = u32_at(info, 16)? as usize;
    let info = &info[..info_size];
    if header >= 0x24 && info.len() >= 32 {
        let unicode_off = u32_at(info, 28)? as usize;
        if unicode_off > 0 && unicode_off < info.len() {
            if let Some(path) = utf16_z(&info[unicode_off..]) {
                return Some(path);
            }
        }
    }
    if local_off == 0 || local_off >= info.len() {
        return None;
    }
    let end = info[local_off..]
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(info.len() - local_off);
    let text = String::from_utf8_lossy(&info[local_off..local_off + end]).into_owned();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg_attr(all(not(windows), not(test)), allow(dead_code))]
fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg_attr(all(not(windows), not(test)), allow(dead_code))]
fn utf16_z(bytes: &[u8]) -> Option<String> {
    let mut units = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        let unit = u16::from_le_bytes([bytes[index], bytes[index + 1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
        index += 2;
        if units.len() > 1024 {
            return None;
        }
    }
    let text = String::from_utf16(&units).ok()?;
    if text.is_empty() { None } else { Some(text) }
}
