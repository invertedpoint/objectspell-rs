//! Connect async components with signals.
//!
//! A component declares what it sends with `#[objectspell::emitter]`, what it is with
//! `#[objectspell::state]`, and what it listens to with `#[objectspell::receiver]`. There is no
//! registry and no callback list: components are wired together **by type name**.
//!
//! That naming rule is the one thing to learn. A receiver block reads
//! `impl Sender for Listener` — "on `Listener`, handle the signals `Sender` sends". The trait
//! position names the component you are listening *to*.
//!
//! [`Connector`] wires a topology, starts it, and emits `connected`. It returns once the
//! topology has shut down, which happens when something calls [`Connector::disconnect`].
//!
//! # Example
//!
//! ```
//! use objectspell::Connector;
//!
//! /// Asks a question.
//! #[objectspell::emitter]
//! pub trait Asker {
//!     /// Emitted once the topology is running.
//!     pub async fn asked(question: String);
//! }
//!
//! /// Asks as soon as everything is connected.
//! #[objectspell::state]
//! pub struct Asker {}
//!
//! #[objectspell::receiver]
//! impl Connector for Asker {
//!     pub async fn connected(&self) {
//!         self.asked("ready?".to_string()).await;
//!     }
//!
//!     pub async fn disconnected(&self) {}
//! }
//!
//! /// Listens for questions.
//! #[objectspell::state]
//! pub struct Answerer {}
//!
//! #[objectspell::receiver]
//! impl Asker for Answerer {
//!     pub async fn asked(&self, question: String) {
//!         println!("heard: {question}");
//!     }
//! }
//!
//! // A receiver on the Connector itself: one question is all this program needed.
//! #[objectspell::receiver]
//! impl Asker for Connector {
//!     pub async fn asked(&self, _question: String) {
//!         self.disconnect().await;
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     Connector::new()
//!         .connect((Asker::default(), Answerer::default()))
//!         .await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Wiring mistakes
//!
//! `connect()` checks the whole topology before starting anything, so a receiver named after
//! nothing, a receiver missing part of its channel, or two components sharing a name are all
//! reported immediately rather than hanging. See [`WiringError`].

#![warn(missing_docs)]

/// Abstracting over typed components so the Connector can hold them together.
pub mod any_state;
/// Starting and stopping a topology.
pub mod connector;
/// The sending half of a component.
pub mod emitter;
/// The receiving half of a component.
pub mod receiver;
/// What travels between components.
pub mod signal;
/// A component's queue, listener, and dispatch.
pub mod state;
/// Checking a topology before it starts.
pub mod wiring;

extern crate self as objectspell;

// Re-export core types
pub use any_state::AnyState;
pub use emitter::EmitterCore;
pub use receiver::SignalDispatcher;
pub use signal::Signal;
pub use state::StateCore;
pub use wiring::WiringError;

pub use connector::Connector;

// Re-export procedural macros
pub use objectspell_macros::{emitter, receiver, state};

// Re-exported so macro-generated code can name these crates without the user
// having to add them to their own manifest.
pub use async_trait::async_trait;
pub use inventory;
pub use tokio;

/// How a receiver registers itself with the State it listens to.
///
/// `#[objectspell::receiver]` submits one of these per receiver impl block. When a State is
/// turned into a running component, every registration whose `target_type` matches that State
/// is applied, which is how receivers are discovered without a hand-written registry.
pub struct DispatcherRegistration {
    /// The component this receiver was written for.
    pub target_type: fn() -> std::any::TypeId,
    /// Attaches the receiver's dispatchers to that component's core.
    pub register: fn(&mut crate::StateCore, std::sync::Arc<dyn std::any::Any + Send + Sync>),
}
inventory::collect!(DispatcherRegistration);

/// What a component declares it sends.
///
/// `#[objectspell::emitter]` submits one of these per emitter block. `connect()` reads them to
/// check that every receiver handles the whole of the channel it is named after.
pub struct EmitterRegistration {
    /// The component whose signals these are.
    pub target_type: fn() -> std::any::TypeId,
    /// The signal names it declares, in declaration order.
    pub routes: &'static [&'static str],
}
inventory::collect!(EmitterRegistration);

/// The routes the given component type declares. Empty if it declares no signals.
pub fn emitter_routes_of(target: std::any::TypeId) -> Vec<&'static str> {
    inventory::iter::<EmitterRegistration>
        .into_iter()
        .filter(|reg| (reg.target_type)() == target)
        .flat_map(|reg| reg.routes.iter().copied())
        .collect()
}
