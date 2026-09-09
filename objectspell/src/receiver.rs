use crate::signal::Signal;

/// Trait for dispatching one signal to one handler method.
///
/// A dispatcher names the channel it listens to (e.g. `"Weather"`) and the route it handles
/// (e.g. `"weather_determined"`). A State indexes its dispatchers by both, so `dispatch` is
/// only ever called for the signal it was registered under and never has to check.
///
/// The `#[objectspell::receiver]` macro can auto-generate this.
#[async_trait::async_trait]
pub trait SignalDispatcher: Send + Sync {
    fn channel(&self) -> &'static str;
    fn route(&self) -> &'static str;
    async fn dispatch(&self, signal: &Signal);
}
