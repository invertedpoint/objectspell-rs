use std::collections::HashMap;

/// A signal sent from an Emitter to connected Receivers.
///
/// - `channel`: names the emitting component (e.g. `"Weather"`, `"News"`)
/// - `route`: names the signal (e.g. `"weather_determined"`, `"completed"`)
/// - `message`: the signal's arguments, keyed by parameter name
#[derive(Debug, Clone)]
pub struct Signal {
    pub channel: String,
    pub route: String,
    pub message: HashMap<String, serde_json::Value>,
}

impl Signal {
    pub fn new(channel: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            route: route.into(),
            message: HashMap::new(),
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl serde::Serialize) -> Self {
        self.message
            .insert(key.into(), serde_json::to_value(value).unwrap());
        self
    }
}
