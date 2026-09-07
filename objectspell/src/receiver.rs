use crate::signal::Signal;

/// Trait for dispatching signals to handler methods.
///
/// Each receiver implementation handles one channel (e.g., "Weather") and
/// routes signals by `signal.route` to the appropriate handler method.
///
/// The `#[objectspell::receiver]` macro can auto-generate this.
#[async_trait::async_trait]
pub trait SignalDispatcher: Send + Sync {
    fn channel(&self) -> &'static str;
    async fn dispatch(&self, signal: &Signal) -> bool;
}
