//! The macros must not emit undocumented public items.
//!
//! A crate may reasonably `deny(missing_docs)`. Anything the macros generate lands in *that*
//! crate, so an undocumented generated item is a warning the user cannot fix, pointing at a
//! line they did not write. Compiling this file at all is the assertion.

#![deny(missing_docs)]

/// Sends one signal.
#[objectspell::emitter]
pub trait Speaker {
    /// Emitted once there is something to say.
    async fn spoke(word: String);
}

/// A component that speaks.
#[objectspell::state]
pub struct Speaker {}

/// A component that listens.
#[objectspell::state]
pub struct Listener {}

#[objectspell::receiver]
impl Speaker for Listener {
    /// Takes down whatever was said.
    async fn spoke(&self, word: String) {
        let _ = word;
    }
}

#[test]
fn the_macros_emit_nothing_undocumented() {
    let _ = Listener::default();
    let _ = Speaker::default();
}
