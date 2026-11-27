//! 文件与注册表采集的按需生命周期。
//!
//! 这里只计订阅并调用宿主的启动和停止。它不加载内核映像，也不覆盖
//! `CatalogHost` 对 `strata-fs` 与 `strata-reg` 的拒绝。进程和网络通道不属于本模块。

use crate::{MAX_SUBSCRIPTIONS, Phase};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Files,
    Registry,
}

impl Channel {
    pub const ALL: [Self; 2] = [Self::Files, Self::Registry];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Registry => "registry",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Files => 0,
            Self::Registry => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectError {
    Unsupported,
    SubscriptionLimit,
    NoSubscription,
    Fault(String),
    Os(i32),
}

impl CollectError {
    pub fn fault(detail: &str) -> Self {
        Self::Fault(safe_detail(detail))
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::SubscriptionLimit => "subscription_limit",
            Self::NoSubscription => "no_subscription",
            Self::Fault(_) => "collector_fault",
            Self::Os(_) => "collector_os",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Unsupported => "This host cannot observe that channel".into(),
            Self::SubscriptionLimit => "Collection subscription limit reached".into(),
            Self::NoSubscription => "Collection channel has no active subscription".into(),
            Self::Fault(detail) => safe_detail(detail),
            Self::Os(code) => format!("Collector reported OS code {code}"),
        }
    }
}

fn safe_detail(detail: &str) -> String {
    if detail.is_empty()
        || detail.len() > 160
        || detail.trim() != detail
        || detail.chars().any(char::is_control)
    {
        "unspecified collector fault".into()
    } else {
        detail.to_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelView {
    pub channel: Channel,
    pub subscriptions: u32,
    pub phase: Phase,
    pub generation: u64,
    pub failure: Option<CollectError>,
}

pub trait CollectHost {
    fn start(&mut self, channel: Channel) -> Result<(), CollectError>;
    fn stop(&mut self, channel: Channel) -> Result<(), CollectError>;
    fn health(&self, channel: Channel) -> Option<CollectError>;
}

#[derive(Clone, Debug)]
struct Module {
    subscriptions: u32,
    phase: Phase,
    generation: u64,
    failure: Option<CollectError>,
}

impl Module {
    fn idle() -> Self {
        Self {
            subscriptions: 0,
            phase: Phase::Idle,
            generation: 0,
            failure: None,
        }
    }

    fn view(&self, channel: Channel) -> ChannelView {
        ChannelView {
            channel,
            subscriptions: self.subscriptions,
            phase: self.phase,
            generation: self.generation,
            failure: self.failure.clone(),
        }
    }
}

pub struct FileRegistrySession<H: CollectHost> {
    host: H,
    modules: [Module; 2],
}

impl<H: CollectHost> FileRegistrySession<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            modules: std::array::from_fn(|_| Module::idle()),
        }
    }

    pub fn view(&self) -> [ChannelView; 2] {
        Channel::ALL.map(|channel| self.modules[channel.index()].view(channel))
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn subscribe(&mut self, channel: Channel) -> Result<ChannelView, CollectError> {
        let index = channel.index();
        let phase = self.modules[index].phase;
        if self.modules[index].subscriptions == MAX_SUBSCRIPTIONS {
            return Err(CollectError::SubscriptionLimit);
        }
        if matches!(phase, Phase::Ready | Phase::UnloadFailed) {
            let module = &mut self.modules[index];
            module.subscriptions += 1;
            if module.phase == Phase::UnloadFailed {
                // 停止失败时采集线程可能仍在；再次订阅不另起一份。
                module.phase = Phase::Ready;
                module.failure = None;
            }
            return Ok(module.view(channel));
        }
        if let Err(error) = self.host.start(channel) {
            let module = &mut self.modules[index];
            module.phase = Phase::Failed;
            module.failure = Some(error.clone());
            module.subscriptions = 0;
            return Err(error);
        }
        let module = &mut self.modules[index];
        module.generation = module.generation.saturating_add(1);
        module.phase = Phase::Ready;
        module.failure = None;
        module.subscriptions += 1;
        Ok(module.view(channel))
    }

    pub fn release(&mut self, channel: Channel) -> Result<ChannelView, CollectError> {
        let index = channel.index();
        if self.modules[index].subscriptions == 0 {
            return Err(CollectError::NoSubscription);
        }
        // 最后一次释放先停止。宿主若 panic，订阅计数仍保持原值。
        if self.modules[index].subscriptions == 1 && self.modules[index].phase == Phase::Ready {
            if let Err(error) = self.host.stop(channel) {
                let module = &mut self.modules[index];
                module.subscriptions = 0;
                module.phase = Phase::UnloadFailed;
                module.failure = Some(error.clone());
                return Err(error);
            }
            let module = &mut self.modules[index];
            module.subscriptions = 0;
            module.phase = Phase::Idle;
            module.failure = None;
            return Ok(module.view(channel));
        }
        self.modules[index].subscriptions -= 1;
        Ok(self.modules[index].view(channel))
    }

    pub fn fault(&mut self, channel: Channel, error: CollectError) {
        let index = channel.index();
        let resident = matches!(
            self.modules[index].phase,
            Phase::Ready | Phase::UnloadFailed
        );
        if resident && self.host.stop(channel).is_err() {
            let module = &mut self.modules[index];
            module.phase = Phase::UnloadFailed;
            module.failure = Some(CollectError::fault("stop failed after a collector fault"));
            module.subscriptions = 0;
            return;
        }
        let module = &mut self.modules[index];
        module.phase = Phase::Failed;
        module.failure = Some(CollectError::fault(&error.detail()));
        module.subscriptions = 0;
    }
}

impl<H: CollectHost> Drop for FileRegistrySession<H> {
    fn drop(&mut self) {
        for channel in Channel::ALL {
            if matches!(
                self.modules[channel.index()].phase,
                Phase::Ready | Phase::UnloadFailed
            ) {
                let _ = self.host.stop(channel);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingHost {
        inner: Arc<Mutex<Record>>,
    }

    #[derive(Default)]
    struct Record {
        start: Vec<Channel>,
        stop: Vec<Channel>,
        fail_start: Option<Channel>,
        fail_stop: bool,
        health: Option<(Channel, CollectError)>,
    }

    impl CollectHost for RecordingHost {
        fn start(&mut self, channel: Channel) -> Result<(), CollectError> {
            let mut record = self.inner.lock().expect("record");
            record.start.push(channel);
            if record.fail_start == Some(channel) {
                return Err(CollectError::fault("callback stopped"));
            }
            Ok(())
        }

        fn stop(&mut self, channel: Channel) -> Result<(), CollectError> {
            let mut record = self.inner.lock().expect("record");
            record.stop.push(channel);
            if record.fail_stop {
                return Err(CollectError::Os(5));
            }
            Ok(())
        }

        fn health(&self, channel: Channel) -> Option<CollectError> {
            let record = self.inner.lock().expect("record");
            record
                .health
                .as_ref()
                .filter(|(failed, _)| *failed == channel)
                .map(|(_, error)| error.clone())
        }
    }

    #[test]
    fn subscriptions_start_once_and_stop_at_zero() {
        let host = RecordingHost::default();
        let record = Arc::clone(&host.inner);
        let mut session = FileRegistrySession::new(host);
        assert_eq!(session.subscribe(Channel::Files).unwrap().subscriptions, 1);
        assert_eq!(session.subscribe(Channel::Files).unwrap().subscriptions, 2);
        assert_eq!(record.lock().unwrap().start, vec![Channel::Files]);
        assert_eq!(session.release(Channel::Files).unwrap().subscriptions, 1);
        assert!(record.lock().unwrap().stop.is_empty());
        let released = session.release(Channel::Files).unwrap();
        assert_eq!(released.phase, Phase::Idle);
        assert_eq!(released.subscriptions, 0);
        assert_eq!(record.lock().unwrap().stop, vec![Channel::Files]);
        assert_eq!(
            session.release(Channel::Files).unwrap_err(),
            CollectError::NoSubscription
        );
    }

    #[test]
    fn one_channel_fault_leaves_the_other_ready() {
        let host = RecordingHost {
            inner: Arc::new(Mutex::new(Record {
                fail_start: Some(Channel::Files),
                ..Record::default()
            })),
        };
        let record = Arc::clone(&host.inner);
        let mut session = FileRegistrySession::new(host);
        assert_eq!(
            session.subscribe(Channel::Files).unwrap_err().code(),
            "collector_fault"
        );
        let ready = session.subscribe(Channel::Registry).unwrap();
        assert_eq!(ready.phase, Phase::Ready);
        assert_eq!(ready.generation, 1);
        assert_eq!(session.view()[Channel::Files.index()].phase, Phase::Failed);
        assert_eq!(session.view()[Channel::Files.index()].subscriptions, 0);
        session.fault(Channel::Registry, CollectError::fault("callback stopped"));
        assert_eq!(
            session.view()[Channel::Registry.index()].phase,
            Phase::Failed
        );
        assert_eq!(session.view()[Channel::Registry.index()].subscriptions, 0);
        assert_eq!(record.lock().unwrap().stop, vec![Channel::Registry]);
        assert_eq!(
            record.lock().unwrap().start,
            vec![Channel::Files, Channel::Registry]
        );
    }

    #[test]
    fn failed_stop_keeps_the_channel_resident_for_the_next_subscription() {
        let host = RecordingHost {
            inner: Arc::new(Mutex::new(Record {
                fail_stop: true,
                ..Record::default()
            })),
        };
        let record = Arc::clone(&host.inner);
        let mut session = FileRegistrySession::new(host);
        session.subscribe(Channel::Registry).unwrap();
        let error = session.release(Channel::Registry).unwrap_err();
        assert_eq!(error, CollectError::Os(5));
        assert_eq!(
            session.view()[Channel::Registry.index()].phase,
            Phase::UnloadFailed
        );
        assert_eq!(session.view()[Channel::Registry.index()].subscriptions, 0);
        let again = session.subscribe(Channel::Registry).unwrap();
        assert_eq!(again.phase, Phase::Ready);
        assert_eq!(again.subscriptions, 1);
        assert_eq!(record.lock().unwrap().start, vec![Channel::Registry]);
    }

    #[test]
    fn subscription_limit_does_not_replace_a_running_channel() {
        let host = RecordingHost::default();
        let record = Arc::clone(&host.inner);
        let mut session = FileRegistrySession::new(host);
        for _ in 0..MAX_SUBSCRIPTIONS {
            session.subscribe(Channel::Files).unwrap();
        }
        assert_eq!(
            session.subscribe(Channel::Files).unwrap_err(),
            CollectError::SubscriptionLimit
        );
        assert_eq!(
            session.view()[Channel::Files.index()].subscriptions,
            MAX_SUBSCRIPTIONS
        );
        assert_eq!(record.lock().unwrap().start.len(), 1);
    }

    #[test]
    fn unsupported_start_does_not_mark_the_other_channel() {
        let host = RecordingHost {
            inner: Arc::new(Mutex::new(Record {
                fail_start: Some(Channel::Registry),
                ..Record::default()
            })),
        };
        let mut session = FileRegistrySession::new(host);
        assert_eq!(
            session.subscribe(Channel::Registry).unwrap_err().code(),
            "collector_fault"
        );
        assert_eq!(session.view()[Channel::Files.index()].phase, Phase::Idle);
        assert_eq!(session.view()[Channel::Files.index()].generation, 0);
    }

    #[test]
    fn drop_stops_a_running_channel_once() {
        let host = RecordingHost::default();
        let record = Arc::clone(&host.inner);
        {
            let mut session = FileRegistrySession::new(host);
            session.subscribe(Channel::Files).unwrap();
            session.subscribe(Channel::Registry).unwrap();
        }
        let stopped = record.lock().unwrap().stop.clone();
        assert_eq!(stopped, vec![Channel::Files, Channel::Registry]);
    }

    #[test]
    fn unsafe_collector_text_is_replaced_before_it_can_leave_the_session() {
        let error = CollectError::fault("line\nbreak");
        assert_eq!(error.detail(), "unspecified collector fault");
        assert_eq!(
            CollectError::Fault("line\nbreak".into()).detail(),
            "unspecified collector fault"
        );
    }
}
