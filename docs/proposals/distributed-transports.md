# Proposal: carry signals between processes

## Status

Proposed. Not scheduled. Large — would land over several releases, and is breaking.

## Problem

`docs/limitations.md` says this is a wiring library, not a message broker, and tells you to use a
broker and let ObjectSpell organise the components around it. That is honest, but it stops one
step short: it does not say *how*. Everyone who follows the advice invents the same things —
an envelope format, a topic naming rule, where to put the partition key, what a signal means when
it reaches three replicas instead of one.

The model is worth extending because the thing it is good at does not stop at a process boundary.
A component declares what it sends and what it listens to, and never names its peers. That is
exactly the shape of a system wired through topics.

## What does not survive the boundary

Four assumptions the current design leans on, all of which break:

- **Arguments are `Arc<dyn Any + Send + Sync>`.** Phase 2a moved deliberately *away* from
  serialised payloads, which is what lets a mismatch be caught by type rather than silently
  defaulted. Nothing can serialise a `dyn Any`.
- **Connect-time validation sees every component.** It cannot see receivers in another process.
- **Wiring by type name is compiler-checked.** Across repositories it becomes a naming contract
  that nothing enforces.
- **A signal reaches every connected receiver.** With three replicas of a service, "every
  receiver" is ambiguous in a way it never was in one process.

A proposal that ignores any of these is not worth implementing.

## Proposed change

### The abstraction lives in the core; the drivers do not

The core gains a `Transport` trait, an envelope type, the remote declaration syntax, and the
schema export. It gains **no new dependencies** — the trait is defined, never implemented, and
`objectspell` stays at four.

A driver crate implements it: `objectspell-kafka` pulls in `rdkafka` and whatever it needs.
Redpanda needs no separate driver, being wire-compatible.

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn publish(&self, envelope: &Envelope) -> Result<(), TransportError>;

    async fn subscribe(
        &self,
        channel: &str,
        delivery: Delivery,
        sink: mpsc::UnboundedSender<Envelope>,
    ) -> Result<(), TransportError>;
}
```

### Encoding happens in generated code, not in the transport

This is the part that decides whether the whole idea works. By the time a signal reaches the
transport its arguments are `Arc<dyn Any>` — type-erased, unserialisable. So a remote emitter
method must encode **at the call site**, where the concrete types are still known, and a remote
dispatcher must decode in the generated handler, where the parameter types are known.

Two representations, converted at the macro boundary:

```rust
/// What crosses the wire.
pub struct Envelope {
    pub channel: String,
    pub route: String,
    pub key: Option<Vec<u8>>,
    pub payload: Vec<u8>,
}
```

The practical consequence: `serde::Serialize + DeserializeOwned` is required **only** on the
parameters of signals declared remote. A user who never declares one pays nothing — no bound, no
dependency, no change. That is the price of readmitting serialisation without undoing 2a.

### Declaring a channel remote

Both ends annotate, because each needs different information.

```rust
// The sender: this channel leaves the process.
#[objectspell::emitter(remote)]
pub trait Weather {
    async fn weather_determined(#[key] region: String, reading: Reading);
}

// The receiver: and what arriving signals mean here.
#[objectspell::receiver(remote, delivery = "queue")]
impl Weather for Archiver {
    async fn weather_determined(&self, region: String, reading: Reading) { … }
}
```

`#[key]` marks the parameter used as the partition key. Signals sharing a key keep their relative
order, which is the closest available analogue to the per-component FIFO ordering
`docs/lifecycle.md` promises locally.

### What a signal means with N replicas

Declared per receiver, because both meanings are legitimate and neither is a safe default for the
other:

| `delivery` | Meaning | Kafka |
|---|---|---|
| `broadcast` (default) | every replica handles every signal | one consumer group per replica |
| `queue` | one replica handles each signal | one group shared across replicas |

`broadcast` is the default because it is what the same code does in one process. Moving a
component from local to remote should not quietly change what its handlers mean; asking for a
work queue should be a thing you wrote down.

### Lifecycle signals stay local, always

`connected` and `disconnected` are never remote, and the `Connector` channel cannot be declared
so. Each process owns its own lifecycle: `connect()` starting or stopping here says nothing about
a process someone else deployed. Propagating shutdown across a cluster is a different feature with
different failure modes, and conflating the two would make `disconnect()` unpredictable.

### Keeping the naming contract honest

Type-name wiring stops being compiler-checked once the peer is in another repository, so
validation has to be replaced with something rather than dropped.

The emitter macro already registers each signal's route; `docs/proposals/runtime-error-reporting.md`
proposes it carry parameter names too. Extending that into an exportable schema — channel, route,
parameters, types, delivery mode — gives a producer and a consumer something to compare in CI,
before either deploys. Connect-time validation then covers local channels as it does now, and the
schema covers the rest.

Without this, a renamed signal becomes a silent production outage. With it, it is a failing build.

### At-least-once changes what a handler must be

In one process a signal is delivered once. Through a broker, with offsets committed after the
handler returns, it is delivered *at least* once: a crash between handling and committing replays
it. Handlers on remote channels must therefore be idempotent, and this belongs in the type system
if it can be made to fit, in the documentation if it cannot.

This is the single largest behavioural difference, and the one most likely to be discovered in
production rather than in review.

### Backpressure stops being optional

Local queues are unbounded because a local producer and consumer run at the same pace, roughly. A
broker will hand over as fast as the network allows, so remote ingestion has to be bounded and
pause consumption rather than grow memory. Fixing this for remote channels is an opportunity to
fix it for local ones too — `docs/limitations.md` currently admits there is no backpressure at all.

## Not in scope

- **Request/reply.** Signals are one-way here and should stay one-way there. Correlation ids and
  reply topics are a different abstraction wearing this one's clothes.
- **Exactly-once delivery.** Achievable with Kafka transactions, at a cost in throughput and
  complexity that the first version should not pay.
- **Cluster-wide shutdown.** See "lifecycle signals stay local".
- **Other brokers.** NATS, AMQP and the rest should fall out of the `Transport` trait if it is
  designed well, but only Kafka and Redpanda would be built.
- **Schema evolution.** Adding a parameter to a signal already in production needs a
  compatibility story. Real, and separable.

## Open questions

- **Does the transport belong in the core at all?** A companion crate with a bridge component
  needs no core changes and no new concepts. The case for the core is that a bridge leaves every
  user to reinvent envelopes, keying and shutdown; the case against is that the core's entire
  pitch is being small, and a `Transport` trait that only one crate implements is an abstraction
  built for an audience of one.
- **Is `broadcast` really the right default?** It preserves local semantics, but most people
  reaching for Kafka want a work queue, and a default that is usually wrong is its own kind of
  trap.
- **How much should validation attempt across the boundary?** A CI-time schema comparison catches
  the common case but needs both sides in one pipeline. A registry would catch more and is
  considerably more machinery.
- **What happens to a remote signal that cannot be encoded?** It is a programming error, but it
  surfaces at run time in the emitting component, which is the one place the current design has
  no error path. This depends on `runtime-error-reporting.md` landing first.
- **Does `#[key]` go far enough?** Ordering guarantees are per key; anything needing a total order
  across a channel cannot have one. That may deserve a louder warning than a doc note.
