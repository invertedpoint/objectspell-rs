//! With the `serde` feature, every emitter registers an encoder for its signals.

#![cfg(feature = "serde")]

use objectspell::encoding::{encoders, EncoderRegistration};
use objectspell::Signal;

#[objectspell::emitter]
pub trait Reporter {
    async fn reported(text: String, count: u32);
    async fn idled();
}

#[objectspell::state]
pub struct Reporter {}

fn reporter() -> &'static EncoderRegistration {
    encoders()
        .find(|registration| registration.channel == "Reporter")
        .expect("Reporter registers an encoder")
}

#[test]
fn routes_and_parameters_are_registered_in_declaration_order() {
    let routes: Vec<(&str, &[&str])> = reporter()
        .routes
        .iter()
        .map(|route| (route.name, route.params))
        .collect();

    assert_eq!(
        routes,
        [("reported", &["text", "count"][..]), ("idled", &[][..])]
    );
}

#[test]
fn arguments_are_encoded_as_a_tuple_in_declaration_order() {
    let signal = Signal::new("Reporter", "reported")
        .with_param("text", "hello".to_string())
        .with_param("count", 3u32);

    let encoded = (reporter().encode)(&signal).unwrap();

    assert_eq!(serde_json::to_string(&encoded).unwrap(), r#"["hello",3]"#);
}

#[test]
fn a_signal_without_arguments_encodes_as_unit() {
    let encoded = (reporter().encode)(&Signal::new("Reporter", "idled")).unwrap();

    assert_eq!(serde_json::to_string(&encoded).unwrap(), "null");
}

#[test]
fn an_argument_of_another_type_is_an_error_not_a_panic() {
    let signal = Signal::new("Reporter", "reported")
        .with_param("text", "hello".to_string())
        .with_param("count", "three".to_string());

    let error = (reporter().encode)(&signal).err().expect("an error");

    assert_eq!(
        error.to_string(),
        "Reporter.reported: no argument `count` of type `u32`"
    );
}

#[test]
fn an_undeclared_route_is_an_error() {
    let error = (reporter().encode)(&Signal::new("Reporter", "vanished"))
        .err()
        .expect("an error");

    assert_eq!(
        error.to_string(),
        "Reporter.vanished: not a signal this channel declares"
    );
}

#[test]
fn the_connector_registers_one_too() {
    // The registration lives in the library's `connector` module. A binary that never calls into
    // that module may not link it (Rust 1.82 drops it), and the registration goes with it.
    let _ = objectspell::Connector::new();

    assert!(encoders().any(|registration| registration.channel == "Connector"));
}

/// Hands a channel to a neighbour: a sender is not `Serialize`.
#[objectspell::emitter]
pub trait Handover {
    async fn handed(link: objectspell::tokio::sync::mpsc::UnboundedSender<u32>);
    async fn said(text: String);
}

#[objectspell::state]
pub struct Handover {}

#[test]
fn every_declared_route_of_a_serialisable_emitter_is_serializable() {
    assert!((reporter().serializable)("reported"));
    assert!((reporter().serializable)("idled"));
    assert!(!(reporter().serializable)("vanished"));
}

#[test]
fn a_route_with_a_parameter_that_is_not_serialize_stays_local() {
    let handover = encoders()
        .find(|registration| registration.channel == "Handover")
        .expect("Handover registers an encoder");
    let (link, _) = objectspell::tokio::sync::mpsc::unbounded_channel::<u32>();

    assert!(!(handover.serializable)("handed"));
    assert!((handover.serializable)("said"));
    let error = (handover.encode)(&Signal::new("Handover", "handed").with_param("link", link))
        .err()
        .expect("an error");
    assert_eq!(
        error.to_string(),
        "Handover.handed: stays local: a parameter is not Serialize"
    );
}
