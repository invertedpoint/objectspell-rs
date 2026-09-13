//! Checking a topology before it starts.
//!
//! Every mistake here is knowable before a signal moves: a receiver named after nothing, a
//! receiver that misses part of its channel, or two components competing for one name. Left
//! unchecked they surface far from their cause — as a component that never hears anything, a
//! line on stderr, or a shutdown that never completes.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use crate::any_state::AnyState;

/// A topology that cannot work, found before anything starts.
pub enum WiringError {
    /// Two connected components share a name, so routing between them is ambiguous.
    DuplicateName {
        /// The name they both have.
        name: String,
    },

    /// A receiver is named after a channel no connected component emits on.
    UnknownChannel {
        /// The component the receiver was written on.
        component: String,
        /// The name the receiver was given.
        channel: String,
        /// Every channel the topology does have, sorted.
        connected: Vec<String>,
    },

    /// A receiver does not handle every signal its channel declares.
    MissingRoutes {
        /// The component the receiver was written on.
        component: String,
        /// The channel it is named after.
        channel: String,
        /// The routes it leaves unhandled, sorted.
        missing: Vec<String>,
    },
}

impl fmt::Display for WiringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateName { name } => write!(
                f,
                "Two connected components are both named '{name}'. Routing is by type name, so \
                 component names must be unique."
            ),
            Self::UnknownChannel {
                component,
                channel,
                connected,
            } => write!(
                f,
                "{component} declares a receiver named '{channel}', but no connected component \
                 emits on that channel. A receiver must be named exactly after the component \
                 whose signals it handles. Connected channels: {}.",
                connected.join(", ")
            ),
            Self::MissingRoutes {
                component,
                channel,
                missing,
            } => write!(
                f,
                "{component}.{channel} does not handle every signal on channel '{channel}': \
                 missing {}. A receiver must define one method per signal its channel declares.",
                missing.join(", ")
            ),
        }
    }
}

/// Deliberately the same as `Display`.
///
/// Returning an error from `main` prints it with `{:?}`, and the whole value of this error is
/// the sentence it carries. The derived form would show the fields as a struct dump in the one
/// place a user is most likely to meet it. Every field appears in the sentence anyway.
impl fmt::Debug for WiringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for WiringError {}

/// Check a topology. Returns the first mistake found, or `Ok(())` if there is none.
///
/// Names are checked first: the channel map the other two checks read is keyed by name, so a
/// duplicate would silently collapse the pair and hide both.
pub async fn validate(states: &[&dyn AnyState]) -> Result<(), WiringError> {
    let mut emitters: HashMap<String, BTreeSet<String>> = HashMap::new();

    for state in states {
        let name = state.emitter_name().await;
        if emitters.contains_key(&name) {
            return Err(WiringError::DuplicateName { name });
        }

        let routes = state
            .emitter_routes()
            .await
            .into_iter()
            .map(str::to_string)
            .collect();
        emitters.insert(name, routes);
    }

    let mut connected: Vec<String> = emitters.keys().cloned().collect();
    connected.sort();

    for state in states {
        let component = state.emitter_name().await;
        let declared = state.declared_routes().await;

        // Sorted, so a component with more than one problem always reports the same one.
        let mut channels: Vec<&String> = declared.keys().collect();
        channels.sort();

        for channel in channels {
            // Cloned rather than moved: the compiler cannot see that returning ends the loop.
            let Some(routes) = emitters.get(channel) else {
                return Err(WiringError::UnknownChannel {
                    component: component.clone(),
                    channel: channel.clone(),
                    connected: connected.clone(),
                });
            };

            let handled = &declared[channel];
            let missing: Vec<String> = routes.difference(handled).cloned().collect();

            if !missing.is_empty() {
                return Err(WiringError::MissingRoutes {
                    component: component.clone(),
                    channel: channel.clone(),
                    missing,
                });
            }
        }
    }

    Ok(())
}
