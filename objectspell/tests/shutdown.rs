//! Stopping a topology: one `disconnected` signal ends every listener.
//!
//! Components are declared at module scope, not inside test functions: the `receiver` macro
//! expands to an `inventory` submission, which needs item position. Because those submissions
//! are shared by the whole test binary, every channel the `Connector` receives here must be
//! present in every topology in this file.

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
async fn run(topology: impl std::future::Future<Output = Result<(), objectspell::WiringError>>) {
    tokio::time::timeout(Duration::from_secs(10), topology)
        .await
        .expect("the topology shut down on its own")
        .expect("the topology wiring is valid");
}

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

    pub async fn disconnected(&self) {}
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
