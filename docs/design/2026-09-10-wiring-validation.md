# Connect-time wiring validation

## Problem

A topology is wired by name: a receiver written `impl Weather for Monitor` is connected to the
component whose type is named `Weather`. Nothing checks that the name matches anything, or that
the receiver handles what that component actually sends. Both mistakes fail late and badly:

- **A name that matches nothing** wires nothing. The receiver waits for a signal no one sends,
  and `connect()` runs until something else stops it — or forever. There is no error.
- **A missing route** is only discovered when that route is first used. Dispatch prints
  `No handler found for Weather.weather_completed` to stderr and carries on, so a component
  silently skips work. When the missing route is one the shutdown path depends on, the topology
  hangs instead.
- **Two components with the same name** make routing ambiguous: a receiver named after them is
  wired to both, and signals interleave from two sources that were meant to be distinct.

All three are mistakes in how a topology is described, and all three are knowable before a single
signal moves. `connect()` should refuse to start and say exactly what is wrong.

## Design

### What `connect()` needs to know

Validation compares two sets per channel: the routes a component **declares** it sends, and the
routes a receiver **handles**. Neither is available at runtime today.

**Routes a component declares.** `#[objectspell::emitter]` and `#[objectspell::state]` are
separate macro invocations on the same type, so the state macro cannot see the emitter's method
names. The emitter macro submits them through `inventory`, mirroring the existing
`DispatcherRegistration` in `lib.rs`:

```rust
pub struct EmitterRegistration {
    pub target_type: fn() -> std::any::TypeId,
    pub routes: &'static [&'static str],
}
inventory::collect!(EmitterRegistration);
```

The registration carries no channel name: a component's channel is its type name, which
`AnyState::emitter_name()` already reports from `EmitterCore`. Storing it twice invites the two
copies to disagree.

The generated `AnyState` wrapper knows its own concrete type, so it reads its registration
directly rather than storing a copy. `emitter_routes_of` is a free function in `lib.rs`, next to
the `inventory::collect!` that makes the registrations findable:

```rust
async fn emitter_routes(&self) -> Vec<&'static str> {
    objectspell::emitter_routes_of(std::any::TypeId::of::<#name>())
}
```

A component with no `#[objectspell::emitter]` block has no registration and declares no routes.
It still appears in the channel map under its own name, with an empty route set.

**Routes a receiver handles, as the user wrote them.** `StateCore::channels()` reports the
finished dispatcher map, which includes the built-in `Connector` receiver injected into every
listening component. Validation must not see that — it would report every component as handling
`Connector.disconnected` whether or not the user wrote anything. `StateCore` therefore keeps a
second, narrower record:

```rust
declared: HashMap<String, BTreeSet<String>>,   // channel -> routes the user declared
```

`register` adds to it only while `connector_installed` is false. `install_connector_receiver`
already sets that flag before registering either built-in, so the two are excluded without a
special case. `AnyState` exposes it as `declared_routes()`.

`BTreeSet` because every message that lists routes lists them in a stable order.

### The three checks

A new module, `objectspell/src/wiring.rs`, holds the error type and one function:

```rust
pub fn validate(states: &[&dyn AnyState]) -> Result<(), WiringError>
```

`connect()` calls it after collecting the states and **before** wiring senders or starting any
listener. A topology that fails validation has done nothing yet.

The slice includes the `Connector` itself, exactly as the wiring loops already see it. That is
what puts `Connector` in the channel map, so a receiver named after it resolves; and it is what
subjects the `Connector`'s own receivers to the same three checks as everyone else's.

1. **`DuplicateName`** — two connected components share a name. Checked first, because the
   channel map the other two checks read is keyed by name and would silently collapse the pair.
2. **`UnknownChannel`** — a component declares a receiver named after a channel no connected
   component emits on. The message lists the connected channels, sorted, because the mistake is
   usually a typo or a component left out of `connect()`.
3. **`MissingRoutes`** — a receiver does not handle every route its channel declares. The message
   lists what is missing.

Check 3 applies to the `Connector` channel like any other. A component that writes no `Connector`
receiver has no `"Connector"` entry in `declared`, so nothing is checked and the built-in serves
it. A component that writes one handling only `connected` is told it is missing `disconnected`.
The rule is uniform: **if you write a receiver, it handles every signal its channel declares.**

Note that check 3 also catches a misspelled handler. A typo does not merely add an unknown name;
it removes a required one, so the missing-route comparison reports it from the other side.

A handler that is *extra* — every declared route handled, plus one more — is not an error. It is
indexed under a route nothing sends and never fires. See "Deliberately out of scope".

### The error type

```rust
#[derive(Debug)]
pub enum WiringError {
    DuplicateName { name: String },
    UnknownChannel { component: String, channel: String, connected: Vec<String> },
    MissingRoutes { component: String, channel: String, missing: Vec<String> },
}
```

`Display` and `std::error::Error` are written by hand — no `thiserror`, so the dependency count
stays at four. Each message names the mistake, where it is, and what to do about it.

### What changes for callers

`connect()` becomes:

```rust
pub async fn connect<C: Connectable>(self, states_tuple: C) -> Result<(), WiringError>
```

Both examples' `main` become `-> Result<(), Box<dyn std::error::Error>>` and call
`connector.connect((x, y)).await?`. This replaces a silent hang with a printed error and a
non-zero exit.

## Components

| Unit | Responsibility | Depends on |
|---|---|---|
| `wiring.rs` | `WiringError`, its `Display`, and `validate` | `AnyState` only |
| `StateCore.declared` | What the user declared, before injection | nothing new |
| `EmitterRegistration` | What each component sends | `inventory` |
| `connect()` | Calls `validate` before doing anything else | `wiring::validate` |

`validate` takes `&[&dyn AnyState]` and returns a value. It touches no channels, spawns nothing,
and can be tested on hand-built states without running a topology.

## Testing

In `objectspell/tests/wiring.rs`, one test per variant, each asserting on the rendered message so
the wording itself is covered:

- a receiver named after nothing → `UnknownChannel`, message lists connected channels
- a `Connector` receiver with only `connected` → `MissingRoutes`, missing `disconnected`
- a receiver on a normal channel missing a route → `MissingRoutes`
- two components with the same name → `DuplicateName`
- a valid topology still connects, runs, and shuts down (regression on phase 2b)
- a failing topology returns before any listener starts — asserted by giving a component a
  handler that records into a shared log, and checking the log is empty after the error

The existing `run()` helper in that file takes a future returning `()` and needs updating for the
new `Result`.

## Deliberately out of scope

- **An extra handler that matches no declared route.** It never fires, silently. This is a stale
  handler left behind by a rename, not a typo — typos are caught by check 3. Worth revisiting,
  but it is a different kind of mistake and no part of this change depends on it.
- **Requiring an emitter to have listeners.** A component may legitimately send signals nobody
  handles yet.
- **Runtime dispatch errors.** `listen` still prints to stderr for an unknown channel or route.
  After this change those paths are unreachable through `connect()`, but they remain the
  behaviour for a `StateCore` driven directly.

## Related proposal

Three handlers in `examples/pubsub` exist only to satisfy check 3 and do nothing:
`App::weather_determined`, `App::something_happened`, and `Connector::started`. The rule that
creates them is the right default — silence about a signal you receive is usually an oversight —
but writing an empty body to say "deliberately ignored" is noise.

A proposal to let a receiver omit handlers it would leave empty is written to
`docs/proposals/optional-no-op-handlers.md`. It is not part of this change: check 3 must exist
before there is anything to relax.
