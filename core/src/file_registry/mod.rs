//! 按需观察一个目录和当前用户的一个注册表键。
//!
//! 订阅从零到一才启动对应通道，最后一次释放才停止。Linux 不能观察 Windows
//! 文件或注册表，启动返回 `unsupported`，不生成事件。已签名的文件系统和注册表
//! 驱动仍然不加载。

#[cfg_attr(not(windows), allow(dead_code))]
mod decode;
#[cfg_attr(not(windows), allow(dead_code))]
mod encode;

#[cfg(windows)]
mod windows;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

pub use winstrata_kernel::{Channel, ChannelView, CollectError, Phase};
use winstrata_kernel::{CollectHost, FileRegistrySession};
use winstrata_protocols::{Event, GapReason, Id};

use crate::EventBus;
use crate::platform;

pub use encode::{FILE_PROVIDER, REGISTRY_PROVIDER};

use decode::{FileNotice, ValueChange};
use encode::{
    Fields, Stream, describe_file, describe_registry, describe_registry_incomplete, gap,
    observation,
};

#[cfg_attr(not(windows), allow(dead_code))]
struct Slot {
    worker: Option<JoinHandle<()>>,
    health: Arc<Mutex<Option<CollectError>>>,
    epoch: u64,
    #[cfg(windows)]
    stop: Option<Arc<windows::StopFlag>>,
}

impl Slot {
    fn new() -> Self {
        Self {
            worker: None,
            health: Arc::new(Mutex::new(None)),
            epoch: 0,
            #[cfg(windows)]
            stop: None,
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
struct Platform {
    files: Slot,
    registry: Slot,
    bus: EventBus,
    device: Id,
    directory: PathBuf,
    subkey: String,
    user: Option<Id>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Platform {
    fn slot(&self, channel: Channel) -> &Slot {
        match channel {
            Channel::Files => &self.files,
            Channel::Registry => &self.registry,
        }
    }

    fn slot_mut(&mut self, channel: Channel) -> &mut Slot {
        match channel {
            Channel::Files => &mut self.files,
            Channel::Registry => &mut self.registry,
        }
    }
}

impl CollectHost for Platform {
    fn start(&mut self, channel: Channel) -> Result<(), CollectError> {
        #[cfg(windows)]
        {
            self.start_channel(channel)
        }
        #[cfg(not(windows))]
        {
            let _ = channel;
            Err(CollectError::Unsupported)
        }
    }

    fn stop(&mut self, channel: Channel) -> Result<(), CollectError> {
        #[cfg(windows)]
        {
            self.stop_channel(channel)
        }
        #[cfg(not(windows))]
        {
            let _ = channel;
            Ok(())
        }
    }

    fn health(&self, channel: Channel) -> Option<CollectError> {
        self.slot(channel)
            .health
            .lock()
            .expect("collector health")
            .clone()
    }
}

pub struct FileRegistry {
    session: FileRegistrySession<Platform>,
}

impl FileRegistry {
    pub fn open(
        bus: EventBus,
        device: Id,
        directory: &Path,
        subkey: &str,
    ) -> Result<Self, CollectError> {
        if !valid_directory(directory) || !valid_subkey(subkey) {
            return Err(CollectError::fault("invalid collection scope"));
        }
        let user = platform::current_id()
            .ok()
            .and_then(|text| Id::new(text).ok());
        Ok(Self {
            session: FileRegistrySession::new(Platform {
                files: Slot::new(),
                registry: Slot::new(),
                bus,
                device,
                directory: directory.to_path_buf(),
                subkey: subkey.to_owned(),
                user,
            }),
        })
    }

    pub fn subscribe(&mut self, channel: Channel) -> Result<ChannelView, CollectError> {
        self.reconcile();
        self.session.subscribe(channel)
    }

    pub fn release(&mut self, channel: Channel) -> Result<ChannelView, CollectError> {
        self.session.release(channel)
    }

    pub fn view(&mut self) -> [ChannelView; 2] {
        self.reconcile();
        self.session.view()
    }

    fn reconcile(&mut self) {
        let failed: Vec<_> = Channel::ALL
            .into_iter()
            .filter_map(|channel| {
                self.session
                    .host()
                    .health(channel)
                    .map(|error| (channel, error))
            })
            .collect();
        for (channel, error) in failed {
            if self.session.view()[channel.index()].phase == Phase::Ready {
                self.session.fault(channel, error);
            }
        }
    }
}

fn valid_directory(path: &Path) -> bool {
    match path.to_str() {
        Some(text)
            if !text.is_empty() && !text.chars().any(char::is_control) && path.is_absolute() =>
        {
            true
        }
        _ => false,
    }
}

fn valid_subkey(subkey: &str) -> bool {
    if subkey.len() > 200 || subkey.starts_with('\\') || subkey.ends_with('\\') {
        return false;
    }
    let mut count = 0;
    for part in subkey.split('\\') {
        count += 1;
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.len() > 255
            || part
                .chars()
                .any(|symbol| symbol.is_control() || symbol == '/' || symbol == ':')
        {
            return false;
        }
    }
    (1..=8).contains(&count)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn unix_ns() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX),
        // 时钟早于 Unix 纪元时保留负值。超出 i64 纳秒范围才饱和，不用 0 代替当前时间。
        Err(error) => -i64::try_from(error.duration().as_nanos()).unwrap_or(i64::MAX),
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(super) struct Publisher {
    bus: EventBus,
    stream: Stream,
    sequence: u64,
    user: Option<Id>,
    health: Arc<Mutex<Option<CollectError>>>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Publisher {
    fn send_fields(&mut self, fields: &Fields, registry_user: bool) -> bool {
        let user = if registry_user {
            self.user.as_ref()
        } else {
            None
        };
        match observation(&self.stream, self.sequence, unix_ns(), fields, user) {
            Ok(event) => self.publish(event),
            Err(error) => {
                self.fail(error);
                false
            }
        }
    }

    fn send_gap(&mut self, reason: GapReason) -> bool {
        match gap(&self.stream, self.sequence, unix_ns(), reason) {
            Ok(event) => self.publish(event),
            Err(error) => {
                self.fail(error);
                false
            }
        }
    }

    fn publish(&mut self, event: Event) -> bool {
        match self.bus.publish(event) {
            Ok(_) => {
                self.sequence = self.sequence.saturating_add(1);
                true
            }
            Err(error) => {
                self.fail(CollectError::fault(error.error.code()));
                false
            }
        }
    }

    fn fail(&self, error: CollectError) {
        let mut health = self.health.lock().expect("collector health");
        if health.is_none() {
            *health = Some(error);
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn emit_files(publisher: &mut Publisher, notices: &[FileNotice], root: &Path) -> bool {
    for notice in notices {
        let observed = observe_path(root, notice_name(notice));
        for fields in describe_file(notice, observed.as_ref()) {
            if !publisher.send_fields(&fields, false) {
                return false;
            }
        }
    }
    true
}

fn notice_name(notice: &FileNotice) -> Option<&str> {
    match notice {
        FileNotice::Added(name)
        | FileNotice::Removed(name)
        | FileNotice::Modified(name)
        | FileNotice::Unpaired { name, .. } => Some(name),
        FileNotice::Renamed { to, .. } => Some(to),
        FileNotice::Undecodable { .. } => None,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn observe_path(root: &Path, name: Option<&str>) -> Option<encode::ObservedPath> {
    let name = name?;
    #[cfg(windows)]
    {
        windows::observe_path(root, name)
    }
    #[cfg(not(windows))]
    {
        let _ = (root, name);
        None
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn emit_registry(
    publisher: &mut Publisher,
    subkey: &str,
    changes: Result<&[ValueChange], ()>,
) -> bool {
    match changes {
        Ok(changes) => {
            for change in changes {
                let fields = describe_registry(subkey, change);
                if !publisher.send_fields(&fields, true) {
                    return false;
                }
            }
            true
        }
        Err(()) => publisher.send_fields(&describe_registry_incomplete(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::{EventBus, Filter, Limits};

    #[test]
    fn linux_and_invalid_scope_publish_nothing() {
        let bus = EventBus::new(Limits::standard()).unwrap();
        let subscription = bus.subscribe(0, Filter::default()).unwrap();
        let mut watch = FileRegistry::open(
            bus,
            Id::new("device-1").unwrap(),
            Path::new("/tmp/winstrata-009-unused"),
            r"Software\WinStrata009",
        )
        .unwrap();
        let error = watch.subscribe(Channel::Files).unwrap_err();
        assert_eq!(error, CollectError::Unsupported);
        assert_eq!(watch.view()[Channel::Files.index()].phase, Phase::Failed);
        assert_eq!(watch.view()[Channel::Registry.index()].phase, Phase::Idle);
        assert!(
            subscription
                .recv_batch(8, Duration::from_millis(30))
                .unwrap()
                .is_empty()
        );
        match FileRegistry::open(
            EventBus::new(Limits::standard()).unwrap(),
            Id::new("device-1").unwrap(),
            Path::new("relative"),
            r"..\Software",
        ) {
            Err(error) => assert_eq!(error.code(), "collector_fault"),
            Ok(_) => panic!("invalid scope was accepted"),
        }
    }
}
