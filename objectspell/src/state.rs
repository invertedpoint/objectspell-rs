use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::receiver::SignalDispatcher;
use crate::signal::Signal;

/// Dispatchers indexed by the channel they listen to, then by the route they handle.
///
/// Indexing on both is what keeps `signal.route` a key rather than something each dispatcher
/// has to compare itself against.
pub type Dispatchers = HashMap<String, HashMap<String, Vec<Box<dyn SignalDispatcher>>>>;

/// The `connected` route of the built-in Connector receiver. Nothing to do by default.
///
/// This is not decoration. A State wired to the Connector receives *every* signal the Connector
/// sends, so without a handler here every State that listens to anything at all would report
/// `Connector.connected` as unhandled. The two names below are the channel and route declared
/// by the `Connector` emitter in `connector.rs`.
struct ConnectedByDefault;

#[async_trait::async_trait]
impl SignalDispatcher for ConnectedByDefault {
    fn channel(&self) -> &'static str {
        "Connector"
    }

    fn route(&self) -> &'static str {
        "connected"
    }

    async fn dispatch(&self, _signal: &Signal) {}
}

/// The `disconnected` route of the built-in Connector receiver: stop this State's listener.
///
/// Every listening State is given one of these, which is why a single `disconnected` signal
/// shuts down a whole topology.
struct StopListening {
    is_stopped: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl SignalDispatcher for StopListening {
    fn channel(&self) -> &'static str {
        "Connector"
    }

    fn route(&self) -> &'static str {
        "disconnected"
    }

    async fn dispatch(&self, _signal: &Signal) {
        self.is_stopped.store(true, Ordering::Relaxed);
    }
}

/// Core state infrastructure. Manages the signal channel and dispatch.
///
/// Each State has:
/// - A channel (unbounded mpsc) for receiving signals
/// - Its dispatchers, indexed by channel and route, for routing signals to handlers
/// - A stop flag, set when the Connector says the topology is shutting down
pub struct StateCore {
    rx: Option<mpsc::UnboundedReceiver<Signal>>,
    tx: mpsc::UnboundedSender<Signal>,
    dispatchers: Dispatchers,
    declared: HashMap<String, BTreeSet<String>>,
    is_stopped: Arc<AtomicBool>,
    connector_installed: bool,
}

impl StateCore {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            rx: Some(rx),
            tx,
            dispatchers: Dispatchers::new(),
            declared: HashMap::new(),
            is_stopped: Arc::new(AtomicBool::new(false)),
            connector_installed: false,
        }
    }

    /// Get a clone of the sender for wiring to emitters.
    pub fn sender(&self) -> mpsc::UnboundedSender<Signal> {
        self.tx.clone()
    }

    /// Register a signal dispatcher under its own channel and route.
    ///
    /// Several dispatchers may share one route: that is how a receiver written by hand and the
    /// built-in Connector receiver both run for the same signal, in registration order.
    pub fn register(&mut self, dispatcher: Box<dyn SignalDispatcher>) {
        let channel = dispatcher.channel().to_string();
        let route = dispatcher.route().to_string();

        // Only what the user wrote. `install_connector_receiver` sets the flag before it
        // registers either built-in, so those are excluded here.
        if !self.connector_installed {
            self.declared
                .entry(channel.clone())
                .or_default()
                .insert(route.clone());
        }

        self.dispatchers
            .entry(channel)
            .or_default()
            .entry(route)
            .or_default()
            .push(dispatcher);
    }

    /// What channels does this state listen to?
    pub fn channels(&self) -> Vec<String> {
        self.dispatchers.keys().cloned().collect()
    }

    /// The receivers this component declares, as the user wrote them: channel to routes.
    ///
    /// Unlike `channels()`, this excludes the built-in Connector receiver, so it reports what
    /// was actually written rather than what was injected.
    pub fn declared_routes(&self) -> &HashMap<String, BTreeSet<String>> {
        &self.declared
    }

    /// Give this State the built-in Connector receiver, so that it can be told to stop.
    ///
    /// Only a State that already listens to something gets one. A State with no receivers never
    /// starts a listener, so there is nothing to stop, and giving it one would make the
    /// Connector wire itself to a component that handles nothing else.
    pub fn install_connector_receiver(&mut self) {
        if self.connector_installed || self.dispatchers.is_empty() {
            return;
        }

        self.connector_installed = true;
        self.register(Box::new(ConnectedByDefault));
        self.register(Box::new(StopListening {
            is_stopped: Arc::clone(&self.is_stopped),
        }));
    }

    /// Start the listener loop, unless this State listens to nothing.
    ///
    /// Returns `None` for a State with no receivers. Such a State holds a sender to its own
    /// queue for as long as it is connected, so a listener started for it would never end and
    /// would keep the whole topology from shutting down.
    pub fn spawn_listener(&mut self) -> Option<JoinHandle<()>> {
        if self.dispatchers.is_empty() {
            return None;
        }

        let rx = self.rx.take()?;
        let dispatchers = std::mem::take(&mut self.dispatchers);
        let is_stopped = Arc::clone(&self.is_stopped);

        Some(tokio::spawn(async move {
            Self::listen(rx, dispatchers, is_stopped).await;
        }))
    }

    /// Process signals until the State is stopped or its queue closes.
    ///
    /// The stop flag is read between signals, never during one: a handler that is already
    /// running always finishes, and everything queued ahead of `disconnected` is handled
    /// before the loop ends.
    async fn listen(
        mut rx: mpsc::UnboundedReceiver<Signal>,
        dispatchers: Dispatchers,
        is_stopped: Arc<AtomicBool>,
    ) {
        while !is_stopped.load(Ordering::Relaxed) {
            let Some(signal) = rx.recv().await else {
                break;
            };

            let Some(routes) = dispatchers.get(&signal.channel) else {
                eprintln!("No receiver found for channel: {}", signal.channel);
                continue;
            };

            let Some(handlers) = routes.get(&signal.route) else {
                eprintln!("No handler found for {}.{}", signal.channel, signal.route);
                continue;
            };

            for handler in handlers {
                handler.dispatch(&signal).await;
            }
        }
    }
}

impl Default for StateCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stands in for a dispatcher the `receiver` macro would generate.
    struct Probe;

    #[async_trait::async_trait]
    impl SignalDispatcher for Probe {
        fn channel(&self) -> &'static str {
            "Pinger"
        }

        fn route(&self) -> &'static str {
            "ping"
        }

        async fn dispatch(&self, _signal: &Signal) {}
    }

    #[test]
    fn register_indexes_a_dispatcher_by_channel_and_route() {
        let mut core = StateCore::new();
        core.register(Box::new(Probe));

        assert_eq!(core.dispatchers["Pinger"]["ping"].len(), 1);
    }

    #[test]
    fn a_core_that_listens_to_nothing_is_given_no_connector() {
        let mut core = StateCore::new();
        core.install_connector_receiver();

        assert!(core.channels().is_empty());
        assert!(core.spawn_listener().is_none());
    }

    #[test]
    fn a_core_that_listens_is_given_both_connector_routes() {
        let mut core = StateCore::new();
        core.register(Box::new(Probe));
        core.install_connector_receiver();

        let connector = &core.dispatchers["Connector"];
        assert_eq!(connector["connected"].len(), 1);
        assert_eq!(connector["disconnected"].len(), 1);
    }

    #[test]
    fn installing_the_connector_twice_installs_it_once() {
        let mut core = StateCore::new();
        core.register(Box::new(Probe));
        core.install_connector_receiver();
        core.install_connector_receiver();

        assert_eq!(core.dispatchers["Connector"]["disconnected"].len(), 1);
    }
}
