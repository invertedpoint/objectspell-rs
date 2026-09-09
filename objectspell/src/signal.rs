use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// A signal sent from an Emitter to connected Receivers.
///
/// - `channel`: names the emitting component (e.g. `"Weather"`, `"News"`)
/// - `route`: names the signal (e.g. `"weather_determined"`, `"completed"`)
/// - `message`: the signal's arguments, keyed by parameter name
///
/// Arguments are stored as-is rather than serialised. A signal is broadcast to every connected
/// receiver, so each argument is held behind an `Arc` and cloned out on the receiving side.
#[derive(Clone)]
pub struct Signal {
    pub channel: String,
    pub route: String,
    pub message: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl Signal {
    /// Create a signal with no arguments.
    pub fn new(channel: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            route: route.into(),
            message: HashMap::new(),
        }
    }

    /// Add one argument, keyed by its parameter name.
    pub fn with_param<T: Any + Send + Sync>(mut self, key: impl Into<String>, value: T) -> Self {
        self.message.insert(key.into(), Arc::new(value));
        self
    }

    /// Read one argument back.
    ///
    /// Returns `None` if there is no argument by that name, or if it holds a different type.
    pub fn param<T: Any + Clone>(&self, name: &str) -> Option<T> {
        self.message.get(name)?.downcast_ref::<T>().cloned()
    }

    /// Read one argument back, panicking if it is missing or holds a different type.
    ///
    /// Both cases mean the emitter and the receiver disagree about the signal, which no
    /// sensible default can paper over. The message names the channel, the route, and the
    /// argument, so the mismatch points at the two declarations that disagree.
    pub fn expect_param<T: Any + Clone>(&self, name: &str) -> T {
        let Some(value) = self.message.get(name) else {
            panic!(
                "signal {}.{} has no argument named `{}`",
                self.channel, self.route, name
            );
        };

        match value.downcast_ref::<T>() {
            Some(value) => value.clone(),
            None => panic!(
                "signal {}.{} carries an argument `{}` of a different type than the `{}` \
                 its handler expects",
                self.channel,
                self.route,
                name,
                std::any::type_name::<T>()
            ),
        }
    }
}

impl std::fmt::Debug for Signal {
    /// Arguments are opaque values, so only their names are shown.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<&str> = self.message.keys().map(String::as_str).collect();
        names.sort_unstable();

        f.debug_struct("Signal")
            .field("channel", &self.channel)
            .field("route", &self.route)
            .field("message", &names)
            .finish()
    }
}
