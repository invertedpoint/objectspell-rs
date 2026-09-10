//! Wiring: linking components and moving signals between them.
//!
//! Components are declared at module scope, not inside test functions: the `receiver` macro
//! expands to an `inventory` submission, which needs item position. Every receiver written for
//! `Connector` is therefore visible to every test in this binary. That is harmless, because a
//! topology only wires the channels it actually contains — a receiver named after a component
//! that is not connected is never reached.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use objectspell::Connector;

/// What a component recorded. Held behind an `Arc` because `connect()` consumes the components,
/// so a test can only read back through something it kept a handle to.
type Log = Arc<Mutex<Vec<String>>>;

fn log() -> Log {
    Arc::new(Mutex::new(Vec::new()))
}

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

/// Run a topology to completion. The timeout is a deadlock guard, not a wait: every topology
/// here shuts down in milliseconds, and one that hangs is exactly the failure being tested for.
async fn run(topology: impl std::future::Future<Output = ()>) {
    tokio::time::timeout(Duration::from_secs(10), topology)
        .await
        .expect("the topology shut down on its own");
}

// ============================================================================
// Registration
// ============================================================================

#[objectspell::emitter]
pub trait Pinger {
    pub async fn ping(value: String);
}

#[objectspell::state]
pub struct Pinger {}

#[objectspell::state]
pub struct Ponger {}

#[objectspell::receiver]
impl Pinger for Ponger {
    pub async fn ping(&self, value: String) {
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

// ============================================================================
// Shutdown
// ============================================================================

#[objectspell::emitter]
pub trait Starter {
    pub async fn started();
}

#[objectspell::state]
pub struct Starter {}

#[objectspell::receiver]
impl Connector for Starter {
    pub async fn connected(&self) {
        self.started().await;
    }
}

#[objectspell::receiver]
impl Starter for Connector {
    pub async fn started(&self) {
        self.disconnect().await;
    }
}

#[tokio::test]
async fn a_receiver_that_disconnects_ends_connect() {
    run(Connector::new().connect((Starter::default(),))).await;
}

/// Writes its own `Connector` receiver, which must not cost it the built-in one.
#[objectspell::state]
pub struct Watcher {
    pub log: Log,
}

#[objectspell::receiver]
impl Connector for Watcher {
    pub async fn connected(&self) {
        self.log.lock().unwrap().push("connected".to_string());
    }

    pub async fn disconnected(&self) {
        self.log.lock().unwrap().push("disconnected".to_string());
    }
}

#[tokio::test]
async fn a_hand_written_connector_receiver_runs_and_the_component_still_stops() {
    let log = log();

    run(Connector::new().connect((Starter::default(), Watcher::init(log.clone())))).await;

    // `connect()` returning is the proof that the built-in handler ran too: it waits on
    // Watcher's listener, which only ends once the stop flag is set.
    assert_eq!(entries(&log), ["connected", "disconnected"]);
}

// ============================================================================
// Ordering
// ============================================================================

#[objectspell::emitter]
pub trait Producer {
    pub async fn item(value: String);
    pub async fn done();
}

#[objectspell::state]
pub struct Producer {}

#[objectspell::receiver]
impl Connector for Producer {
    pub async fn connected(&self) {
        for value in 0..5 {
            self.item(value.to_string()).await;
        }
        self.done().await;
    }
}

#[objectspell::state]
pub struct Consumer {
    pub log: Log,
}

#[objectspell::receiver]
impl Producer for Consumer {
    pub async fn item(&self, value: String) {
        self.log.lock().unwrap().push(value);
    }

    pub async fn done(&self) {}
}

#[objectspell::receiver]
impl Producer for Connector {
    pub async fn item(&self, _value: String) {}

    pub async fn done(&self) {
        self.disconnect().await;
    }
}

#[tokio::test]
async fn work_queued_before_disconnect_is_still_handled() {
    let log = log();

    // Producer sends all five items and `done` before the Connector sees `done`, so
    // `disconnected` reaches Consumer's queue behind work it has not started yet.
    run(Connector::new().connect((Producer::default(), Consumer::init(log.clone())))).await;

    assert_eq!(entries(&log), ["0", "1", "2", "3", "4"]);
}
