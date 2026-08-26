use aether_core::runtime::CoreConfig;
use aether_core::{AetherCore, CoreState};

#[test]
fn core_completes_full_lifecycle() {
    let config = CoreConfig::default();

    let mut core = AetherCore::new(config);

    assert_eq!(core.state(), CoreState::Created);

    core.initialize().expect("initialization must succeed");

    assert_eq!(core.state(), CoreState::Initialized);

    assert!(core.identity().is_some());

    assert!(core.device().is_some());

    assert!(core.device_key().is_some());

    let health = core.health();

    assert!(health.is_healthy());

    core.start().expect("start must succeed");

    assert_eq!(core.state(), CoreState::Running);

    core.stop().expect("stop must succeed");

    assert_eq!(core.state(), CoreState::Stopped);
}

#[test]
fn core_cannot_start_before_initialization() {
    let mut core = AetherCore::new(CoreConfig::default());

    assert!(core.start().is_err());
}

#[test]
fn core_cannot_initialize_twice() {
    let mut core = AetherCore::new(CoreConfig::default());

    core.initialize().expect("first initialization must work");

    assert!(core.initialize().is_err());
}
