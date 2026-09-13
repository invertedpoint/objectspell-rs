//! How the macros treat what you write.
//!
//! Declared in a nested module on purpose: a sender that came out private would still be
//! callable from the module that declared it, so the mistake only shows from outside.

mod elsewhere {
    #[objectspell::emitter]
    pub trait Quiet {
        /// Declared with no visibility, the way a real trait method is.
        async fn plain();
    }

    #[objectspell::state]
    pub struct Quiet {}

    #[objectspell::emitter]
    pub trait Loud {
        /// Declared `pub`, which stays supported.
        async fn explicit();
    }

    #[objectspell::state]
    pub struct Loud {}
}

#[tokio::test]
async fn a_signal_declared_without_pub_is_reachable() {
    elsewhere::Quiet::default().plain().await;
}

#[tokio::test]
async fn a_signal_declared_with_pub_is_reachable() {
    elsewhere::Loud::default().explicit().await;
}
