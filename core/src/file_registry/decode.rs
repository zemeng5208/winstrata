//! 把 Windows 文件通知缓冲和注册表值快照解码成与操作系统无关的记录。
//!
//! 缓冲布局与 `FILE_NOTIFY_INFORMATION` 相同：小端 `NextEntryOffset`、`Action`、
//! `FileNameLength`，随后是按字节计的 UTF-16 文件名。动作值使用公开的 `FILE_ACTION_*`。

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileNotice {
    Added(String),
    Removed(String),
    Modified(String),
    Renamed { from: String, to: String },
    Unpaired { name: String, old: bool },
    Undecodable { action: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
}

pub const ACTION_ADDED: u32 = 1;
pub const ACTION_REMOVED: u32 = 2;
pub const ACTION_MODIFIED: u32 = 3;
pub const ACTION_RENAMED_OLD: u32 = 4;
pub const ACTION_RENAMED_NEW: u32 = 5;

/// Win32 `ERROR_NOTIFY_ENUM_DIR`。缓冲装不下时目录变更被操作系统丢弃。
pub const ERROR_NOTIFY_ENUM_DIR: i32 = 1022;
/// Win32 `ERROR_OPERATION_ABORTED`。取消未完成的重叠读取时返回。
pub const ERROR_OPERATION_ABORTED: i32 = 995;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyRead {
    Ready(u32),
    Lost,
    Aborted,
    Failed(i32),
}

pub fn classify_notify(code: i32, transferred: u32) -> NotifyRead {
    if code == ERROR_OPERATION_ABORTED {
        NotifyRead::Aborted
    } else if code == ERROR_NOTIFY_ENUM_DIR || (code == 0 && transferred == 0) {
        NotifyRead::Lost
    } else if code == 0 {
        NotifyRead::Ready(transferred)
    } else {
        NotifyRead::Failed(code)
    }
}

pub fn decode_file_notifications(bytes: &[u8]) -> Result<Vec<FileNotice>, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError::Truncated);
    }
    let mut notices = Vec::new();
    let mut pending_old: Option<Result<String, ()>> = None;
    let mut offset = 0usize;
    loop {
        let rest = bytes
            .len()
            .checked_sub(offset)
            .ok_or(DecodeError::Truncated)?;
        if rest < 12 {
            return Err(DecodeError::Truncated);
        }
        let next = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let action = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let name_bytes =
            u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
        let name_end = offset
            .checked_add(12)
            .and_then(|start| start.checked_add(name_bytes))
            .ok_or(DecodeError::Truncated)?;
        if name_bytes % 2 != 0 || name_end > bytes.len() {
            return Err(DecodeError::Truncated);
        }
        let name = utf16_name(&bytes[offset + 12..name_end]);
        // 旧名称只有紧随的新名称才能配成一次重命名，否则先交出未配对记录。
        if action != ACTION_RENAMED_NEW {
            flush_pending(&mut notices, &mut pending_old, true);
        }
        match action {
            ACTION_RENAMED_OLD => pending_old = Some(name),
            ACTION_RENAMED_NEW => {
                if let Some(old) = pending_old.take() {
                    match (old, name) {
                        (Ok(from), Ok(to)) => notices.push(FileNotice::Renamed { from, to }),
                        _ => notices.push(FileNotice::Undecodable { action }),
                    }
                } else {
                    push_named(&mut notices, action, name, true);
                }
            }
            _ => push_named(&mut notices, action, name, false),
        }
        if next == 0 {
            break;
        }
        let step = next as usize;
        if step < 12 || offset.checked_add(step).is_none_or(|end| end > bytes.len()) {
            return Err(DecodeError::Truncated);
        }
        offset += step;
        if offset == bytes.len() {
            break;
        }
    }
    flush_pending(&mut notices, &mut pending_old, true);
    Ok(notices)
}

fn utf16_name(bytes: &[u8]) -> Result<String, ()> {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect();
    String::from_utf16(&units).map_err(|_| ())
}

fn push_named(
    notices: &mut Vec<FileNotice>,
    action: u32,
    name: Result<String, ()>,
    unpaired_new: bool,
) {
    let Ok(name) = name else {
        notices.push(FileNotice::Undecodable { action });
        return;
    };
    notices.push(match action {
        ACTION_ADDED => FileNotice::Added(name),
        ACTION_REMOVED => FileNotice::Removed(name),
        ACTION_MODIFIED => FileNotice::Modified(name),
        ACTION_RENAMED_NEW if unpaired_new => FileNotice::Unpaired { name, old: false },
        _ => FileNotice::Undecodable { action },
    });
}

fn flush_pending(
    notices: &mut Vec<FileNotice>,
    pending: &mut Option<Result<String, ()>>,
    force: bool,
) {
    if !force {
        return;
    }
    if let Some(old) = pending.take() {
        match old {
            Ok(name) => notices.push(FileNotice::Unpaired { name, old: true }),
            Err(()) => notices.push(FileNotice::Undecodable {
                action: ACTION_RENAMED_OLD,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueSample {
    pub name: String,
    pub kind: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueChange {
    Added(ValueSample),
    Removed(ValueSample),
    Changed {
        before: ValueSample,
        after: ValueSample,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffError {
    Duplicate,
}

pub fn diff_values(
    before: &[ValueSample],
    after: &[ValueSample],
) -> Result<Vec<ValueChange>, DiffError> {
    let mut prior = index(before)?;
    let mut current = index(after)?;
    let mut names: Vec<&str> = prior.keys().chain(current.keys()).copied().collect();
    names.sort_unstable();
    names.dedup();
    let mut changes = Vec::new();
    for name in names {
        match (prior.remove(name), current.remove(name)) {
            (None, Some(sample)) => changes.push(ValueChange::Added(sample.clone())),
            (Some(sample), None) => changes.push(ValueChange::Removed(sample.clone())),
            (Some(old), Some(new)) if old.kind != new.kind || old.data != new.data => {
                changes.push(ValueChange::Changed {
                    before: old.clone(),
                    after: new.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(changes)
}

fn index(
    samples: &[ValueSample],
) -> Result<std::collections::BTreeMap<&str, &ValueSample>, DiffError> {
    let mut map = std::collections::BTreeMap::new();
    for sample in samples {
        if map.insert(sample.name.as_str(), sample).is_some() {
            return Err(DiffError::Duplicate);
        }
    }
    Ok(map)
}

#[cfg(test)]
pub fn notification_bytes(entries: &[(u32, &str)]) -> Vec<u8> {
    let mut buffer = Vec::new();
    for (index, (action, name)) in entries.iter().enumerate() {
        let wide: Vec<u8> = name
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();
        let start = buffer.len();
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&action.to_le_bytes());
        buffer.extend_from_slice(&(wide.len() as u32).to_le_bytes());
        buffer.extend_from_slice(&wide);
        while buffer.len() % 4 != 0 {
            buffer.push(0);
        }
        let next = if index + 1 == entries.len() {
            0
        } else {
            u32::try_from(buffer.len() - start).unwrap()
        };
        buffer[start..start + 4].copy_from_slice(&next.to_le_bytes());
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_notifications_keep_actions_names_and_rename_pairs() {
        let bytes = notification_bytes(&[
            (ACTION_ADDED, "a.txt"),
            (ACTION_MODIFIED, "a.txt"),
            (ACTION_RENAMED_OLD, "a.txt"),
            (ACTION_RENAMED_NEW, "b.txt"),
            (ACTION_REMOVED, "b.txt"),
        ]);
        let notices = decode_file_notifications(&bytes).unwrap();
        assert_eq!(
            notices,
            vec![
                FileNotice::Added("a.txt".into()),
                FileNotice::Modified("a.txt".into()),
                FileNotice::Renamed {
                    from: "a.txt".into(),
                    to: "b.txt".into(),
                },
                FileNotice::Removed("b.txt".into()),
            ]
        );
    }

    #[test]
    fn truncated_or_unpaired_notifications_stay_explicit() {
        assert_eq!(
            decode_file_notifications(&[0, 0, 0, 0, 1, 0, 0, 0]),
            Err(DecodeError::Truncated)
        );
        let short = notification_bytes(&[(ACTION_ADDED, "a")]);
        assert_eq!(
            decode_file_notifications(&short[..13]),
            Err(DecodeError::Truncated)
        );
        assert_eq!(classify_notify(ERROR_NOTIFY_ENUM_DIR, 0), NotifyRead::Lost);
        assert_eq!(classify_notify(0, 0), NotifyRead::Lost);
        assert_eq!(classify_notify(0, 12), NotifyRead::Ready(12));
        assert_eq!(
            classify_notify(ERROR_OPERATION_ABORTED, 0),
            NotifyRead::Aborted
        );
        let old = notification_bytes(&[(ACTION_RENAMED_OLD, "old.txt")]);
        assert_eq!(
            decode_file_notifications(&old).unwrap(),
            vec![FileNotice::Unpaired {
                name: "old.txt".into(),
                old: true,
            }]
        );
        let mut broken = notification_bytes(&[(ACTION_ADDED, "a")]);
        let name_at = 12;
        broken[name_at] = 0xFF;
        broken[name_at + 1] = 0xDF;
        assert_eq!(
            decode_file_notifications(&broken).unwrap(),
            vec![FileNotice::Undecodable {
                action: ACTION_ADDED
            }]
        );
    }

    #[test]
    fn value_diff_reports_only_real_changes() {
        let before = vec![
            ValueSample {
                name: "keep".into(),
                kind: 4,
                data: vec![1, 0, 0, 0],
            },
            ValueSample {
                name: "edit".into(),
                kind: 4,
                data: vec![1, 0, 0, 0],
            },
            ValueSample {
                name: "gone".into(),
                kind: 1,
                data: vec![0],
            },
        ];
        let after = vec![
            ValueSample {
                name: "edit".into(),
                kind: 4,
                data: vec![2, 0, 0, 0],
            },
            ValueSample {
                name: "keep".into(),
                kind: 4,
                data: vec![1, 0, 0, 0],
            },
            ValueSample {
                name: "new".into(),
                kind: 4,
                data: vec![3, 0, 0, 0],
            },
        ];
        assert_eq!(
            diff_values(&before, &after).unwrap(),
            vec![
                ValueChange::Changed {
                    before: before[1].clone(),
                    after: after[0].clone(),
                },
                ValueChange::Removed(before[2].clone()),
                ValueChange::Added(after[2].clone()),
            ]
        );
        let duplicate = vec![before[0].clone(), before[0].clone()];
        assert_eq!(
            diff_values(&duplicate, &after).unwrap_err(),
            DiffError::Duplicate
        );
    }
}
