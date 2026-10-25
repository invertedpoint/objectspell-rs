//! Encoders for every emitter's signals, so they can leave the process.
//!
//! Only with the `serde` feature. Every `#[objectspell::emitter]` then registers an
//! [`EncoderRegistration`](crate::encoding::EncoderRegistration) whose `encode` turns one of its
//! signals into a value any serde serialiser accepts: the arguments as a tuple, in declaration
//! order. The format is the caller's choice.
//!
//! A route whose parameters are not all `serde::Serialize` gets no encoder: `serializable`
//! reports it, and `encode` returns an error for it. That is decided where each emitter is
//! declared, by autoref specialization on the tuple of its argument types, so components that
//! hand each other channels or handles still compile.

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;

use crate::signal::Signal;

/// A signal's arguments, ready for any serde serialiser.
pub type Encoded = Box<dyn erased_serde::Serialize + Send + Sync>;

/// One signal's name and its parameter names, in declaration order.
pub struct RouteParams {
    /// The signal's name.
    pub name: &'static str,
    /// Its parameter names.
    pub params: &'static [&'static str],
}

/// What each emitter registers when the `serde` feature is on.
pub struct EncoderRegistration {
    /// The emitting component's name.
    pub channel: &'static str,
    /// Its signals, in declaration order.
    pub routes: &'static [RouteParams],
    /// Whether this declared signal's parameters are all `Serialize`; `false` for any other name.
    pub serializable: fn(&str) -> bool,
    /// One of its signals, as a serialisable tuple of arguments.
    pub encode: fn(&Signal) -> Result<Encoded, EncodeError>,
}

inventory::collect!(EncoderRegistration);

/// Every encoder compiled into this binary.
pub fn encoders() -> impl Iterator<Item = &'static EncoderRegistration> {
    inventory::iter::<EncoderRegistration>.into_iter()
}

/// A signal that could not be encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeError {
    /// The channel it was on.
    pub channel: String,
    /// The signal's name.
    pub route: String,
    /// What went wrong.
    pub reason: String,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}: {}", self.channel, self.route, self.reason)
    }
}

impl std::error::Error for EncodeError {}

/// Read one argument for encoding. A missing or differently typed argument is an error.
pub fn argument<T: Any + Clone>(signal: &Signal, name: &str) -> Result<T, EncodeError> {
    signal.param::<T>(name).ok_or_else(|| EncodeError {
        channel: signal.channel.clone(),
        route: signal.route.clone(),
        reason: format!(
            "no argument `{name}` of type `{}`",
            std::any::type_name::<T>()
        ),
    })
}

/// The error for a signal its channel does not declare.
pub fn unknown_route(signal: &Signal) -> EncodeError {
    EncodeError {
        channel: signal.channel.clone(),
        route: signal.route.clone(),
        reason: "not a signal this channel declares".to_string(),
    }
}

/// The error for a signal with a parameter that is not `Serialize`.
pub fn local_route(signal: &Signal) -> EncodeError {
    EncodeError {
        channel: signal.channel.clone(),
        route: signal.route.clone(),
        reason: "stays local: a parameter is not Serialize".to_string(),
    }
}

/// Asks, where `__encoders!` expands, whether the argument tuple `T` is `Serialize`.
///
/// Called as `(&Probe::<T>(PhantomData)).boxed(..)`. Method lookup tries `Probe<T>` before
/// `&Probe<T>`, so [`ViaSerde`] wins whenever its bound holds, and [`LocalOnly`] answers
/// otherwise. `T` is concrete at the call, so this needs no unstable specialization.
#[doc(hidden)]
pub struct Probe<T>(pub PhantomData<T>);

/// The answer for a `Serialize` argument tuple.
#[doc(hidden)]
pub trait ViaSerde {
    /// The argument tuple.
    type Args;
    /// Whether the tuple can be encoded.
    fn serializable(&self) -> bool;
    /// The tuple, boxed for encoding, if it can be.
    fn boxed(&self, args: Self::Args) -> Option<Encoded>;
}

impl<T: serde::Serialize + Send + Sync + 'static> ViaSerde for Probe<T> {
    type Args = T;

    fn serializable(&self) -> bool {
        true
    }

    fn boxed(&self, args: T) -> Option<Encoded> {
        Some(Box::new(args))
    }
}

/// The answer for any other argument tuple.
#[doc(hidden)]
pub trait LocalOnly {
    /// The argument tuple.
    type Args;
    /// Whether the tuple can be encoded.
    fn serializable(&self) -> bool;
    /// The tuple, boxed for encoding, if it can be.
    fn boxed(&self, args: Self::Args) -> Option<Encoded>;
}

impl<T> LocalOnly for &Probe<T> {
    type Args = T;

    fn serializable(&self) -> bool {
        false
    }

    fn boxed(&self, _args: T) -> Option<Encoded> {
        None
    }
}
