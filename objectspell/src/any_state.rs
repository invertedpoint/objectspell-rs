use tokio::task::JoinHandle;

use crate::signal::Signal;

/// Trait to allow abstracting over typed States in the Connector.
/// `#[objectspell::state]` implements this on a generated wrapper around `Arc<Mutex<T>>`.
#[async_trait::async_trait]
pub trait AnyState: Send + Sync {
    /// Start the signal listening loop.
    async fn start_listener(&self) -> Option<JoinHandle<()>>;

    /// The sender half of the state's channel. Used by emitters to broadcast signals here.
    async fn sender(&self) -> Option<tokio::sync::mpsc::UnboundedSender<Signal>>;

    /// The channels this state is listening to.
    async fn channels(&self) -> Vec<String>;

    /// The name of the channel this state's emitter broadcasts as.
    async fn emitter_name(&self) -> String;

    /// The routes this component declares it sends.
    async fn emitter_routes(&self) -> Vec<&'static str>;

    /// Broadcast a signal from this state's emitter to all connected receivers.
    async fn broadcast(&self, signal: Signal);

    /// Connect a receiver (another state's sender) to this state's emitter.
    async fn connect_receiver(&self, sender: tokio::sync::mpsc::UnboundedSender<Signal>);
}
