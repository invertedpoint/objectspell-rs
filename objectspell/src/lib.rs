pub mod any_state;
pub mod connector;
pub mod emitter;
pub mod receiver;
pub mod signal;
pub mod state;

extern crate self as objectspell;

// Re-export core types
pub use any_state::AnyState;
pub use emitter::EmitterCore;
pub use receiver::SignalDispatcher;
pub use signal::Signal;
pub use state::StateCore;

pub use connector::Connector;

// Re-export procedural macros
pub use objectspell_macros::{emitter, receiver, state};

// Re-exported so macro-generated code can name these crates without the user
// having to add them to their own manifest.
pub use async_trait::async_trait;
pub use inventory;
pub use serde;
pub use serde_json;
pub use tokio;

/// How a receiver registers itself with the State it listens to.
///
/// `#[objectspell::receiver]` submits one of these per receiver impl block. When a State is
/// turned into a running component, every registration whose `target_type` matches that State
/// is applied, which is how receivers are discovered without a hand-written registry.
pub struct DispatcherRegistration {
    pub target_type: fn() -> std::any::TypeId,
    pub register: fn(&mut crate::StateCore, std::sync::Arc<dyn std::any::Any + Send + Sync>),
}
inventory::collect!(DispatcherRegistration);
