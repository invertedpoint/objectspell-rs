//! Wiring validation: the three mistakes `connect()` refuses to start with.
//!
//! These call `validate` directly rather than `connect()`, so a broken topology needs no
//! shutdown path and no `Connector` receiver.

use objectspell::wiring::{validate, WiringError};
use objectspell::AnyState;

#[objectspell::emitter]
pub trait Source {
    pub async fn first();
    pub async fn second();
}

#[objectspell::state]
pub struct Source {}

/// Handles everything `Source` declares.
#[objectspell::state]
pub struct GoodSink {}

#[objectspell::receiver]
impl Source for GoodSink {
    pub async fn first(&self) {}
    pub async fn second(&self) {}
}

/// Handles only half of what `Source` declares.
#[objectspell::state]
pub struct PartialSink {}

#[objectspell::receiver]
impl Source for PartialSink {
    pub async fn first(&self) {}
}

/// Named after a component that does not exist — the shape of a typo.
///
/// `Sauce` is deliberately never declared as a type. The `receiver` macro takes the channel name
/// from the trait path and then discards the path, so a receiver can name a channel that nothing
/// defines — which is exactly the mistake this check exists to catch.
#[objectspell::state]
pub struct TypoSink {}

#[objectspell::receiver]
impl Sauce for TypoSink {
    pub async fn first(&self) {}
}

#[tokio::test]
async fn a_correct_topology_validates() {
    let source = Source::default().into_state();
    let sink = GoodSink::default().into_state();

    let states: Vec<&dyn AnyState> = vec![source.as_ref(), sink.as_ref()];
    assert!(validate(&states).await.is_ok());
}

#[tokio::test]
async fn a_receiver_missing_a_route_is_rejected() {
    let source = Source::default().into_state();
    let sink = PartialSink::default().into_state();

    let states: Vec<&dyn AnyState> = vec![source.as_ref(), sink.as_ref()];
    let error = validate(&states).await.unwrap_err();

    assert!(matches!(error, WiringError::MissingRoutes { .. }));
    assert_eq!(
        error.to_string(),
        "PartialSink.Source does not handle every signal on channel 'Source': missing second. \
         A receiver must define one method per signal its channel declares."
    );
}

#[tokio::test]
async fn a_receiver_named_after_nothing_is_rejected() {
    let source = Source::default().into_state();
    let sink = TypoSink::default().into_state();

    let states: Vec<&dyn AnyState> = vec![source.as_ref(), sink.as_ref()];
    let error = validate(&states).await.unwrap_err();

    assert!(matches!(error, WiringError::UnknownChannel { .. }));
    assert_eq!(
        error.to_string(),
        "TypoSink declares a receiver named 'Sauce', but no connected component emits on that \
         channel. A receiver must be named exactly after the component whose signals it handles. \
         Connected channels: Source, TypoSink."
    );
}

#[tokio::test]
async fn two_components_with_the_same_name_are_rejected() {
    let first = Source::default().into_state();
    let second = Source::default().into_state();

    let states: Vec<&dyn AnyState> = vec![first.as_ref(), second.as_ref()];
    let error = validate(&states).await.unwrap_err();

    assert!(matches!(error, WiringError::DuplicateName { .. }));
    assert_eq!(
        error.to_string(),
        "Two connected components are both named 'Source'. Routing is by type name, so \
         component names must be unique."
    );
}
