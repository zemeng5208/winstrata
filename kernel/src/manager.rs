pub const MAX_SUBSCRIPTIONS: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Driver {
    Fs,
    Reg,
    Proc,
    Net,
}

impl Driver {
    pub const ALL: [Self; 4] = [Self::Fs, Self::Reg, Self::Proc, Self::Net];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fs => "strata-fs",
            Self::Reg => "strata-reg",
            Self::Proc => "strata-proc",
            Self::Net => "strata-net",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|driver| driver.as_str() == value)
    }

    fn index(self) -> usize {
        match self {
            Self::Fs => 0,
            Self::Reg => 1,
            Self::Proc => 2,
            Self::Net => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Ready,
    Failed,
    UnloadFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostError {
    ImageAbsent,
    SubscriptionLimit,
    NoSubscription,
    Fault(String),
    Os(i32),
}

impl HostError {
    pub fn fault(detail: &str) -> Self {
        Self::Fault(safe_detail(detail))
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::ImageAbsent => "image_absent",
            Self::SubscriptionLimit => "subscription_limit",
            Self::NoSubscription => "no_subscription",
            Self::Fault(_) => "driver_fault",
            Self::Os(_) => "driver_os",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::ImageAbsent => "No signed driver image is available for this module".into(),
            Self::SubscriptionLimit => "Driver subscription limit reached".into(),
            Self::NoSubscription => "Driver has no active subscription".into(),
            Self::Fault(detail) => safe_detail(detail),
            Self::Os(code) => format!("Host reported OS code {code}"),
        }
    }
}

fn safe_detail(detail: &str) -> String {
    if detail.is_empty()
        || detail.len() > 160
        || detail.trim() != detail
        || detail.chars().any(char::is_control)
    {
        "unspecified host fault".into()
    } else {
        detail.to_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriverView {
    pub driver: Driver,
    pub subscriptions: u32,
    pub phase: Phase,
    pub generation: u64,
    pub failure: Option<HostError>,
}

pub trait DriverHost {
    fn load(&mut self, driver: Driver) -> Result<(), HostError>;
    fn unload(&mut self, driver: Driver) -> Result<(), HostError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CatalogHost;

impl DriverHost for CatalogHost {
    fn load(&mut self, driver: Driver) -> Result<(), HostError> {
        let _ = driver;
        // 没有已签名映像时不得创建或启动内核服务。
        Err(HostError::ImageAbsent)
    }

    fn unload(&mut self, driver: Driver) -> Result<(), HostError> {
        let _ = driver;
        Err(HostError::ImageAbsent)
    }
}

#[derive(Clone, Debug)]
struct Module {
    subscriptions: u32,
    phase: Phase,
    generation: u64,
    failure: Option<HostError>,
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

    fn view(&self, driver: Driver) -> DriverView {
        DriverView {
            driver,
            subscriptions: self.subscriptions,
            phase: self.phase,
            generation: self.generation,
            failure: self.failure.clone(),
        }
    }
}

pub struct Manager<H> {
    host: H,
    modules: [Module; 4],
}

impl<H: DriverHost> Manager<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            modules: std::array::from_fn(|_| Module::idle()),
        }
    }

    pub fn view(&self) -> [DriverView; 4] {
        Driver::ALL.map(|driver| self.modules[driver.index()].view(driver))
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn subscribe(&mut self, driver: Driver) -> Result<DriverView, HostError> {
        let index = driver.index();
        let phase = self.modules[index].phase;
        if self.modules[index].subscriptions == MAX_SUBSCRIPTIONS {
            return Err(HostError::SubscriptionLimit);
        }
        if matches!(phase, Phase::Ready | Phase::UnloadFailed) {
            let module = &mut self.modules[index];
            module.subscriptions += 1;
            if module.phase == Phase::UnloadFailed {
                // 卸载失败说明映像仍可能驻留；新的订阅不重复加载。
                module.phase = Phase::Ready;
                module.failure = None;
            }
            return Ok(module.view(driver));
        }
        if let Err(error) = self.host.load(driver) {
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
        Ok(module.view(driver))
    }

    pub fn release(&mut self, driver: Driver) -> Result<DriverView, HostError> {
        let index = driver.index();
        if self.modules[index].subscriptions == 0 {
            return Err(HostError::NoSubscription);
        }
        // 最后一次释放先卸载。宿主若 panic，订阅计数仍保持原值。
        if self.modules[index].subscriptions == 1 && self.modules[index].phase == Phase::Ready {
            if let Err(error) = self.host.unload(driver) {
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
            return Ok(module.view(driver));
        }
        self.modules[index].subscriptions -= 1;
        Ok(self.modules[index].view(driver))
    }

    pub fn fault(&mut self, driver: Driver, error: HostError) {
        let index = driver.index();
        let resident = matches!(
            self.modules[index].phase,
            Phase::Ready | Phase::UnloadFailed
        );
        if resident && self.host.unload(driver).is_err() {
            let module = &mut self.modules[index];
            module.phase = Phase::UnloadFailed;
            module.failure = Some(HostError::fault("unload failed after a driver fault"));
            module.subscriptions = 0;
            return;
        }
        let module = &mut self.modules[index];
        module.phase = Phase::Failed;
        module.failure = Some(HostError::fault(&error.detail()));
        module.subscriptions = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingHost {
        load: Vec<Driver>,
        unload: Vec<Driver>,
        fail_load: Option<Driver>,
        fail_unload: bool,
    }

    impl DriverHost for RecordingHost {
        fn load(&mut self, driver: Driver) -> Result<(), HostError> {
            self.load.push(driver);
            if self.fail_load == Some(driver) {
                return Err(HostError::fault("callback stopped"));
            }
            Ok(())
        }

        fn unload(&mut self, driver: Driver) -> Result<(), HostError> {
            self.unload.push(driver);
            if self.fail_unload {
                return Err(HostError::Os(5));
            }
            Ok(())
        }
    }

    #[test]
    fn catalog_refuses_every_module_without_calling_a_loader() {
        let mut manager = Manager::new(CatalogHost);
        for driver in Driver::ALL {
            let error = manager.subscribe(driver).unwrap_err();
            assert_eq!(error, HostError::ImageAbsent);
            let views = manager.view();
            let view = &views[driver.index()];
            assert_eq!(view.phase, Phase::Failed);
            assert_eq!(view.subscriptions, 0);
            assert_eq!(view.generation, 0);
        }
        assert_eq!(
            manager.subscribe(Driver::Fs).unwrap_err(),
            HostError::ImageAbsent
        );
        assert_eq!(manager.view()[Driver::Reg.index()].phase, Phase::Failed);
    }

    #[test]
    fn subscriptions_load_once_and_unload_at_zero() {
        let mut manager = Manager::new(RecordingHost::default());
        assert_eq!(manager.subscribe(Driver::Proc).unwrap().subscriptions, 1);
        assert_eq!(manager.subscribe(Driver::Proc).unwrap().subscriptions, 2);
        assert_eq!(manager.host().load, vec![Driver::Proc]);
        assert_eq!(manager.release(Driver::Proc).unwrap().subscriptions, 1);
        assert!(manager.host().unload.is_empty());
        let released = manager.release(Driver::Proc).unwrap();
        assert_eq!(released.phase, Phase::Idle);
        assert_eq!(released.subscriptions, 0);
        assert_eq!(manager.host().unload, vec![Driver::Proc]);
        assert_eq!(
            manager.release(Driver::Proc).unwrap_err(),
            HostError::NoSubscription
        );
    }

    #[test]
    fn one_module_fault_leaves_the_other_ready() {
        let mut manager = Manager::new(RecordingHost {
            fail_load: Some(Driver::Fs),
            ..RecordingHost::default()
        });
        assert_eq!(
            manager.subscribe(Driver::Fs).unwrap_err().code(),
            "driver_fault"
        );
        let ready = manager.subscribe(Driver::Net).unwrap();
        assert_eq!(ready.phase, Phase::Ready);
        assert_eq!(ready.generation, 1);
        assert_eq!(manager.view()[Driver::Fs.index()].phase, Phase::Failed);
        assert_eq!(manager.view()[Driver::Fs.index()].subscriptions, 0);
        manager.fault(Driver::Net, HostError::fault("callback stopped"));
        assert_eq!(manager.view()[Driver::Net.index()].phase, Phase::Failed);
        assert_eq!(manager.view()[Driver::Net.index()].subscriptions, 0);
        assert_eq!(manager.host().unload, vec![Driver::Net]);
        let view = manager.view();
        assert_eq!(
            view[Driver::Fs.index()].failure.as_ref().unwrap().code(),
            "driver_fault"
        );
    }

    #[test]
    fn failed_unload_keeps_the_module_resident_for_the_next_subscription() {
        let mut manager = Manager::new(RecordingHost {
            fail_unload: true,
            ..RecordingHost::default()
        });
        manager.subscribe(Driver::Reg).unwrap();
        let error = manager.release(Driver::Reg).unwrap_err();
        assert_eq!(error, HostError::Os(5));
        assert_eq!(
            manager.view()[Driver::Reg.index()].phase,
            Phase::UnloadFailed
        );
        assert_eq!(manager.view()[Driver::Reg.index()].subscriptions, 0);
        let again = manager.subscribe(Driver::Reg).unwrap();
        assert_eq!(again.phase, Phase::Ready);
        assert_eq!(again.subscriptions, 1);
        assert_eq!(manager.host().load, vec![Driver::Reg]);
    }

    #[test]
    fn subscription_limit_does_not_replace_a_ready_module() {
        let mut manager = Manager::new(RecordingHost::default());
        for _ in 0..MAX_SUBSCRIPTIONS {
            manager.subscribe(Driver::Proc).unwrap();
        }
        assert_eq!(
            manager.subscribe(Driver::Proc).unwrap_err(),
            HostError::SubscriptionLimit
        );
        assert_eq!(
            manager.view()[Driver::Proc.index()].subscriptions,
            MAX_SUBSCRIPTIONS
        );
        assert_eq!(manager.host().load.len(), 1);
    }

    #[test]
    fn unsafe_host_text_is_replaced_before_it_can_leave_the_manager() {
        let error = HostError::fault("line\nbreak");
        assert_eq!(error.detail(), "unspecified host fault");
        assert_eq!(
            HostError::Fault("line\nbreak".into()).detail(),
            "unspecified host fault"
        );
    }
}
