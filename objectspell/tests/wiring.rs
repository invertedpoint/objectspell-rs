//! Registration: what a component declares, and what it is given.
//!
//! Components are declared at module scope, not inside test functions: the `receiver` macro
//! expands to an `inventory` submission, which needs item position.

#[objectspell::emitter]
pub trait Pinger {
    async fn ping(value: String);
}

#[objectspell::state]
pub struct Pinger {}

#[objectspell::state]
pub struct Ponger {}

#[objectspell::receiver]
impl Pinger for Ponger {
    async fn ping(&self, value: String) {
        let _ = value;
    }
}

#[tokio::test]
async fn a_receiver_declared_in_a_test_binary_is_registered() {
    let ponger = Ponger::default().into_state();

    let mut channels = ponger.channels().await;
    channels.sort();

    // `Connector` is the built-in receiver every listening component is given, so that one
    // `disconnected` signal can stop it.
    assert_eq!(channels, ["Connector", "Pinger"]);
}

#[tokio::test]
async fn a_component_that_listens_to_nothing_gets_no_listener() {
    let pinger = Pinger::default().into_state();

    assert!(pinger.channels().await.is_empty());
    assert!(pinger.start_listener().await.is_none());
}

#[tokio::test]
async fn a_component_reports_the_routes_it_declares() {
    let pinger = Pinger::default().into_state();
    assert_eq!(pinger.emitter_routes().await, ["ping"]);
}

#[tokio::test]
async fn a_component_with_no_emitter_block_declares_no_routes() {
    let ponger = Ponger::default().into_state();
    assert!(ponger.emitter_routes().await.is_empty());
}

#[tokio::test]
async fn declared_routes_exclude_the_built_in_connector_receiver() {
    let ponger = Ponger::default().into_state();

    // `channels()` sees the injected built-in; `declared_routes()` must not.
    let mut channels = ponger.channels().await;
    channels.sort();
    assert_eq!(channels, ["Connector", "Pinger"]);

    let declared = ponger.declared_routes().await;
    assert_eq!(declared.keys().collect::<Vec<_>>(), ["Pinger"]);
    assert_eq!(declared["Pinger"].iter().collect::<Vec<_>>(), ["ping"]);
}
