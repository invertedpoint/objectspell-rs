use tokio::sync::mpsc;

use crate::signal::Signal;

/// Core emitter infrastructure. Broadcasts signals to connected receivers.
///
/// Each emitter has a channel name — the name of the component it belongs to — and a list of
/// senders, one per receiver connected to it.
pub struct EmitterCore {
    pub channel_name: String,
    senders: Vec<mpsc::UnboundedSender<Signal>>,
}

impl EmitterCore {
    pub fn new(channel_name: impl Into<String>) -> Self {
        Self {
            channel_name: channel_name.into(),
            senders: Vec::new(),
        }
    }

    /// Connect a receiver to this emitter by providing its sender.
    pub fn connect(&mut self, sender: mpsc::UnboundedSender<Signal>) {
        self.senders.push(sender);
    }

    /// Broadcast a signal to all connected receivers.
    pub fn broadcast(&self, signal: Signal) {
        for sender in &self.senders {
            let _ = sender.send(signal.clone());
        }
    }
}
