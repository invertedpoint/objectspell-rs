use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::receiver::SignalDispatcher;
use crate::signal::Signal;

/// Core state infrastructure. Manages the signal channel and dispatch.
///
/// Each State has:
/// - A channel (unbounded mpsc) for receiving signals
/// - A map of channel → dispatcher for routing signals
pub struct StateCore {
    pub rx: Option<mpsc::UnboundedReceiver<Signal>>,
    pub tx: mpsc::UnboundedSender<Signal>,
    pub dispatchers: HashMap<String, Vec<Box<dyn SignalDispatcher>>>,
}

impl StateCore {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            rx: Some(rx),
            tx,
            dispatchers: HashMap::new(),
        }
    }

    /// Get a clone of the sender for wiring to emitters.
    pub fn sender(&self) -> mpsc::UnboundedSender<Signal> {
        self.tx.clone()
    }

    /// Register a signal dispatcher for a channel.
    pub fn register(&mut self, dispatcher: Box<dyn SignalDispatcher>) {
        let channel = dispatcher.channel().to_string();
        self.dispatchers
            .entry(channel)
            .or_insert_with(Vec::new)
            .push(dispatcher);
    }

    /// Take the receiver to start the listener loop.
    pub fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<Signal>> {
        self.rx.take()
    }

    /// What channels does this state listen to?
    pub fn channels(&self) -> Vec<String> {
        self.dispatchers.keys().cloned().collect()
    }

    /// Run the listener loop — processes signals until the channel is closed.
    pub async fn listen(
        mut rx: mpsc::UnboundedReceiver<Signal>,
        dispatchers: &HashMap<String, Vec<Box<dyn SignalDispatcher>>>,
    ) {
        while let Some(signal) = rx.recv().await {
            if let Some(channel_dispatchers) = dispatchers.get(&signal.channel) {
                let mut handled = false;
                for dispatcher in channel_dispatchers {
                    if dispatcher.dispatch(&signal).await {
                        handled = true;
                    }
                }

                if !handled {
                    eprintln!("No handler found for {}.{}", signal.channel, signal.route);
                }
            } else {
                eprintln!("No receiver found for channel: {}", signal.channel);
            }
        }
    }
}

impl Default for StateCore {
    fn default() -> Self {
        Self::new()
    }
}
