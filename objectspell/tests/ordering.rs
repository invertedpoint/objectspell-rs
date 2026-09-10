//! Ordering: work queued before `disconnected` is still handled.
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

    pub async fn disconnected(&self) {}
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
