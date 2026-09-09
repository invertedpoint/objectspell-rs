//! Wiring: linking components and moving signals between them.
//!
//! Components are declared at module scope, not inside test functions: the `receiver` macro
//! expands to an `inventory` submission, which needs item position.

#[objectspell::emitter]
pub trait Pinger {
    pub async fn ping(value: String);
}

#[objectspell::state]
pub struct Pinger {}

#[objectspell::emitter]
pub trait Ponger {}

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
    assert_eq!(ponger.channels().await, vec!["Pinger".to_string()]);
}
