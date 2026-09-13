//! Wiring validation: the three mistakes `connect()` refuses to start with.
//!
//! These call `validate` directly rather than `connect()`, so a broken topology needs no
//! shutdown path and no `Connector` receiver.

use std::time::Duration;

use objectspell::wiring::{validate, WiringError};
use objectspell::AnyState;

#[objectspell::emitter]
pub trait Source {
    async fn first();
    async fn second();
}

#[objectspell::state]
pub struct Source {}

/// Handles everything `Source` declares.
#[objectspell::state]
pub struct GoodSink {}

#[objectspell::receiver]
impl Source for GoodSink {
    async fn first(&self) {}
    async fn second(&self) {}
}

/// Handles only half of what `Source` declares.
#[objectspell::state]
pub struct PartialSink {}

#[objectspell::receiver]
impl Source for PartialSink {
    async fn first(&self) {}
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
    async fn first(&self) {}
}

/// Writes a `Connector` receiver, but only half of one.
#[objectspell::state]
pub struct HalfConnector {}

#[objectspell::receiver]
impl Connector for HalfConnector {
    async fn connected(&self) {}
    // `disconnected` is missing.
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

#[tokio::test]
async fn the_error_reads_the_same_however_it_is_printed() {
    let source = Source::default().into_state();
    let sink = PartialSink::default().into_state();

    let states: Vec<&dyn AnyState> = vec![source.as_ref(), sink.as_ref()];
    let error = validate(&states).await.unwrap_err();

    // Returning an error from `main` prints it with `{:?}`, which is where a user is most
    // likely to meet it.
    assert_eq!(format!("{error:?}"), error.to_string());
}

#[tokio::test]
async fn connect_refuses_a_broken_topology() {
    let error = objectspell::Connector::new()
        .connect((Source::default(), PartialSink::default()))
        .await
        .unwrap_err();

    assert!(matches!(error, WiringError::MissingRoutes { .. }));
}

#[tokio::test]
async fn a_half_written_connector_receiver_is_rejected() {
    let error = objectspell::Connector::new()
        .connect((HalfConnector::default(),))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "HalfConnector.Connector does not handle every signal on channel 'Connector': \
         missing disconnected. A receiver must define one method per signal its channel declares."
    );
}

#[tokio::test]
async fn a_rejected_topology_starts_nothing() {
    // Nothing in this topology can emit `disconnected`, so had listeners been started,
    // `connect()` would still be awaiting their handles. Returning at all is the proof that
    // validation ran first.
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        objectspell::Connector::new().connect((Source::default(), PartialSink::default())),
    )
    .await
    .expect("connect() returned instead of blocking on listeners that never end");

    assert!(matches!(result.unwrap_err(), WiringError::MissingRoutes { .. }));
}
