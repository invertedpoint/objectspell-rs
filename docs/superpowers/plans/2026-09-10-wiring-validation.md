# Wiring Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** `connect()` refuses to start a topology whose wiring is wrong, and says exactly what
is wrong, instead of hanging or printing to stderr and carrying on.

**Architecture:** Two facts are made available at connect time — the routes each component
declares it sends (via a new `EmitterRegistration` submitted through `inventory`, mirroring the
existing `DispatcherRegistration`), and the routes each component's receivers handle *as the user
wrote them* (a new `declared` map on `StateCore`, populated by `register` only before the
built-in Connector receiver is injected). A new `objectspell/src/wiring.rs` compares the two and
returns a `WiringError`. `connect()` calls it before wiring senders or starting any listener.

**Tech Stack:** Rust 1.82, tokio, async-trait, inventory, proc macros (syn/quote).

**Spec:** `docs/superpowers/specs/2026-09-10-wiring-validation-design.md` (committed at `2067cb9`)

**Before Task 1:** copy this plan to `docs/superpowers/plans/2026-09-10-wiring-validation.md`
and commit it. Work on a branch off `main`: `git checkout -b wiring-validation`.

## Global Constraints

- **No mention of the reference implementation anywhere in either repo** — not in rustdoc, code
  comments, `docs/`, or commit messages. Verify with
  `grep -rniE "python|objectspell-py" --include="*.rs" --include="*.md" --include="*.toml" .`
  (excluding `./target`), which must find nothing.
- **Dependency count stays at four**: `objectspell-macros`, `tokio`, `async-trait`, `inventory`.
  No `thiserror` — write `Display` and `std::error::Error` by hand.
- `cargo clippy --workspace --all-targets -- -D warnings` must be clean after every task.
- Commit messages are **one line**, no body, no trailers.
- `rust-version = "1.82"` — no newer language features.

---

### Task 1: Report the routes each component declares

**Files:**
- Modify: `objectspell/src/lib.rs` (add `EmitterRegistration`, `emitter_routes_of`)
- Modify: `objectspell/src/any_state.rs` (add `emitter_routes` to the trait)
- Modify: `objectspell-macros/src/lib.rs` (emitter macro submits; state macro implements)
- Test: `objectspell/tests/wiring.rs`

**Interfaces:**
- Produces: `objectspell::EmitterRegistration { target_type: fn() -> TypeId, routes: &'static [&'static str] }`;
  `objectspell::emitter_routes_of(TypeId) -> Vec<&'static str>`;
  `AnyState::emitter_routes(&self) -> Vec<&'static str>` (async).

- [ ] **Step 1: Write the failing test**

Append to `objectspell/tests/wiring.rs`:

```rust
#[tokio::test]
async fn a_component_reports_the_routes_it_declares() {
    let pinger = Pinger::default().into_state();
    assert_eq!(pinger.emitter_routes().await, ["ping"]);
}

#[tokio::test]
async fn a_component_with_no_emitter_block_declares_no_routes() {
    let ponger = Ponger::default().into_state();
    assert!(ponger.emitter_routes().await.is_empty());
}
```

`Ponger` currently has an empty `#[objectspell::emitter] pub trait Ponger {}` block. Delete that
block — it generates an empty `impl Ponger {}` and nothing else uses it — so the second test
covers the genuinely-no-emitter case.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test wiring`
Expected: FAIL — `no method named 'emitter_routes' found`.

- [ ] **Step 3: Add the registration type and lookup**

In `objectspell/src/lib.rs`, after the existing `DispatcherRegistration` block:

```rust
/// What a component declares it sends.
///
/// `#[objectspell::emitter]` submits one of these per emitter block. `connect()` reads them to
/// check that every receiver handles the whole of the channel it is named after.
pub struct EmitterRegistration {
    pub target_type: fn() -> std::any::TypeId,
    pub routes: &'static [&'static str],
}
inventory::collect!(EmitterRegistration);

/// The routes the given component type declares. Empty if it declares no signals.
pub fn emitter_routes_of(target: std::any::TypeId) -> Vec<&'static str> {
    inventory::iter::<EmitterRegistration>
        .into_iter()
        .filter(|reg| (reg.target_type)() == target)
        .flat_map(|reg| reg.routes.iter().copied())
        .collect()
}
```

- [ ] **Step 4: Submit a registration from the emitter macro**

In `objectspell-macros/src/lib.rs`, in `pub fn emitter`, collect the route names alongside the
generated methods. Add `let mut route_names = Vec::new();` next to `let mut generated_methods =
Vec::new();`, then inside the `for method in block.methods` loop, immediately after
`let sig_name_str = sig_name.to_string();`, add:

```rust
        route_names.push(sig_name_str.clone());
```

Then change the final `let gen = quote! { ... }` to:

```rust
    let gen = quote! {
        impl #name {
            #(#generated_methods)*
        }

        objectspell::inventory::submit! {
            objectspell::EmitterRegistration {
                target_type: || std::any::TypeId::of::<#name>(),
                routes: &[#(#route_names),*],
            }
        }
    };
```

- [ ] **Step 5: Add the trait method and its generated body**

In `objectspell/src/any_state.rs`, add to the `AnyState` trait:

```rust
    /// The routes this component declares it sends.
    async fn emitter_routes(&self) -> Vec<&'static str>;
```

In `objectspell-macros/src/lib.rs`, in `pub fn state`, inside the
`impl objectspell::AnyState for #wrapper_name` block, add:

```rust
            async fn emitter_routes(&self) -> Vec<&'static str> {
                objectspell::emitter_routes_of(std::any::TypeId::of::<#name>())
            }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy clean.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Record the routes each component declares it sends"
```

---

### Task 2: Report the receiver routes a user actually wrote

**Files:**
- Modify: `objectspell/src/state.rs` (add `declared` field, populate in `register`, expose it)
- Modify: `objectspell/src/any_state.rs` (add `declared_routes` to the trait)
- Modify: `objectspell-macros/src/lib.rs` (state macro implements it)
- Test: `objectspell/tests/wiring.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `StateCore::declared_routes(&self) -> &HashMap<String, BTreeSet<String>>`;
  `AnyState::declared_routes(&self) -> HashMap<String, BTreeSet<String>>` (async, cloned).

**Why a second map:** `StateCore::channels()` reports the finished dispatcher map, which includes
the built-in Connector receiver injected into every listening component. Validation must see only
what the user wrote, or every component would appear to handle `Connector.disconnected`.
`install_connector_receiver` already sets `connector_installed = true` *before* registering either
built-in, so guarding on that flag excludes them without a special case.

- [ ] **Step 1: Write the failing test**

Append to `objectspell/tests/wiring.rs`:

```rust
#[tokio::test]
async fn declared_routes_exclude_the_built_in_connector_receiver() {
    let ponger = Ponger::default().into_state();

    // `channels()` sees the injected built-in; `declared_routes()` must not.
    let mut channels = ponger.channels().await;
    channels.sort();
    assert_eq!(channels, ["Connector", "Pinger"]);

    let declared = ponger.declared_routes().await;
    assert_eq!(declared.keys().collect::<Vec<_>>(), ["Pinger"]);
    assert_eq!(
        declared["Pinger"].iter().collect::<Vec<_>>(),
        ["ping"]
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test wiring`
Expected: FAIL — `no method named 'declared_routes' found`.

- [ ] **Step 3: Record declarations in `StateCore`**

In `objectspell/src/state.rs`, add `BTreeSet` to the collections import
(`use std::collections::{BTreeSet, HashMap};`), then add the field to the struct:

```rust
    declared: HashMap<String, BTreeSet<String>>,
```

Initialise it in `new()` with `declared: HashMap::new(),`, and change `register` to:

```rust
    pub fn register(&mut self, dispatcher: Box<dyn SignalDispatcher>) {
        let channel = dispatcher.channel().to_string();
        let route = dispatcher.route().to_string();

        // Only what the user wrote. `install_connector_receiver` sets the flag before it
        // registers either built-in, so those are excluded here.
        if !self.connector_installed {
            self.declared
                .entry(channel.clone())
                .or_default()
                .insert(route.clone());
        }

        self.dispatchers
            .entry(channel)
            .or_default()
            .entry(route)
            .or_default()
            .push(dispatcher);
    }
```

Add the accessor next to `channels()`:

```rust
    /// The receivers this component declares, as the user wrote them: channel to routes.
    ///
    /// Unlike `channels()`, this excludes the built-in Connector receiver, so it reports what
    /// was actually written rather than what was injected.
    pub fn declared_routes(&self) -> &HashMap<String, BTreeSet<String>> {
        &self.declared
    }
```

- [ ] **Step 4: Expose it through `AnyState`**

In `objectspell/src/any_state.rs`, add the import
`use std::collections::{BTreeSet, HashMap};` and the trait method:

```rust
    /// The receivers this component declares, as the user wrote them: channel to routes.
    async fn declared_routes(&self) -> HashMap<String, BTreeSet<String>>;
```

In `objectspell-macros/src/lib.rs`, in the `impl objectspell::AnyState for #wrapper_name` block:

```rust
            async fn declared_routes(&self) -> std::collections::HashMap<String, std::collections::BTreeSet<String>> {
                self.inner.lock().await.state_core.declared_routes().clone()
            }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Record receiver routes as the user declared them"
```

---

### Task 3: The validator and its error type

**Files:**
- Create: `objectspell/src/wiring.rs`
- Modify: `objectspell/src/lib.rs` (`pub mod wiring;` and re-export `WiringError`)
- Test: `objectspell/tests/validation.rs` (new file)

**Interfaces:**
- Consumes: `AnyState::emitter_name`, `emitter_routes` (Task 1), `declared_routes` (Task 2).
- Produces: `objectspell::WiringError` (enum, 3 variants);
  `objectspell::wiring::validate(&[&dyn AnyState]) -> Result<(), WiringError>` (async).

**Why a separate test binary:** `#[objectspell::receiver]` submits to `inventory`, which is
per **test binary**, not per test. Components declared in one `tests/*.rs` file are invisible to
another, which keeps each file's topologies independent. These tests call `validate` directly, so
they need no `Connector` receiver and no shutdown.

- [ ] **Step 1: Write the failing tests**

Create `objectspell/tests/validation.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test validation`
Expected: FAIL — `unresolved import objectspell::wiring`.

- [ ] **Step 3: Write the module**

Create `objectspell/src/wiring.rs`:

```rust
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
#[derive(Debug)]
pub enum WiringError {
    /// Two connected components share a name, so routing between them is ambiguous.
    DuplicateName { name: String },

    /// A receiver is named after a channel no connected component emits on.
    UnknownChannel {
        component: String,
        channel: String,
        connected: Vec<String>,
    },

    /// A receiver does not handle every signal its channel declares.
    MissingRoutes {
        component: String,
        channel: String,
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
```

- [ ] **Step 4: Export it**

In `objectspell/src/lib.rs`, add `pub mod wiring;` to the module list and
`pub use wiring::WiringError;` to the re-exports.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test validation && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Add wiring validation and its error type"
```

---

### Task 4: Validate in `connect()`

**Files:**
- Modify: `objectspell/src/connector.rs` (signature, call `validate`)
- Modify: `examples/simple/src/main.rs`, `examples/pubsub/src/main.rs` (only these two — every
  receiver in both examples already handles its whole channel, verified against the current tree)
- Split: `objectspell/tests/wiring.rs` into `wiring.rs`, `shutdown.rs`, `ordering.rs`
- Test: `objectspell/tests/validation.rs` (add the end-to-end test)

**Interfaces:**
- Consumes: `wiring::validate` (Task 3).
- Produces: `Connector::connect<C: Connectable>(self, C) -> Result<(), WiringError>` (async).

**The test-binary constraint:** `inventory` is per test binary. `tests/wiring.rs` currently
registers two receivers on the shared `objectspell::Connector` type — `impl Starter for Connector`
and `impl Producer for Connector`. Once `connect()` validates, the `Starter`-only topology sees a
Connector declaring a receiver named `Producer` with no `Producer` connected, and fails with
`UnknownChannel`. Splitting the file gives each topology its own inventory. **The rule to hold:
every channel the `Connector` receives must be present in every topology in that binary.**

- [ ] **Step 1: Write the failing end-to-end test**

Append to `objectspell/tests/validation.rs`. Add `use std::time::Duration;` to its imports.

```rust
/// Writes a `Connector` receiver, but only half of one.
#[objectspell::state]
pub struct HalfConnector {}

#[objectspell::receiver]
impl Connector for HalfConnector {
    pub async fn connected(&self) {}
    // `disconnected` is missing.
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
```

`a_half_written_connector_receiver_is_rejected` is the case the rule was chosen for: the built-in
would have supplied `disconnected`, and the check rejects it anyway. It has to go through
`connect()` rather than `validate()` directly, because only `connect()` puts the `Connector` in
the topology; without it the channel `Connector` would be *unknown* rather than *incomplete*.

`HalfConnector` appears in exactly one topology, so its gap does not affect the other tests. The
binary registers no `impl … for Connector`, so the `Connector`'s own `declared_routes()` is empty
and it contributes nothing to any check.

Do **not** add `use objectspell::Connector;` to this file. In `impl Connector for HalfConnector`
the macro discards the trait path, and `objectspell::Connector::new()` is written in full — an
import would be unused and `-D warnings` would reject it.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test validation`
Expected: FAIL — `no method named 'unwrap_err'` on `()`.

- [ ] **Step 3: Change `connect()`**

In `objectspell/src/connector.rs`, change the signature and add the call. The body is otherwise
unchanged; only the three marked lines are new:

```rust
    pub async fn connect<C: Connectable>(self, states_tuple: C) -> Result<(), crate::WiringError> {
        let connector = self.into_state();

        let mut states_vec = Vec::new();
        states_tuple.into_states(&mut states_vec);

        let mut states_slice_vec: Vec<&dyn AnyState> = vec![connector.as_ref()];
        states_slice_vec.extend(states_vec.iter().map(|s| s.as_ref()));
        let states_slice = states_slice_vec.as_slice();

        // NEW: nothing has been wired or started yet, so a rejected topology leaves no trace.
        crate::wiring::validate(states_slice).await?;

        // The four numbered blocks that follow — collect senders, wire them to emitters, start
        // listeners, broadcast `connected` — are untouched. Do not retype them; leave the
        // existing lines exactly as they are and only add the two lines marked NEW.

        for handle in handles {
            let _ = handle.await;
        }

        Ok(()) // NEW
    }
```

Update the doc comment's first line to mention the check:
`/// Check the wiring, connect every component to the emitters it listens to, start them, and emit `connected`.`

- [ ] **Step 4: Update both examples' `main`**

`examples/simple/src/main.rs` and `examples/pubsub/src/main.rs`, same shape in both:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = Connector::new();
    // ... component construction unchanged ...

    connector.connect((x, y)).await?;

    Ok(())
}
```

- [ ] **Step 5: Split the test file**

`objectspell/tests/wiring.rs` keeps only the registration tests: the `Pinger`/`Ponger`
components, `a_receiver_declared_in_a_test_binary_is_registered`,
`a_component_that_listens_to_nothing_gets_no_listener`, and the four tests added in Tasks 1 and 2.
Delete the `Log`/`log()`/`entries()`/`run()` helpers from it — nothing left in that file calls
`connect()`.

Create `objectspell/tests/shutdown.rs`, moving `Starter`, `Watcher`, their receivers, and the two
shutdown tests. Add the missing no-op to `Starter`'s Connector receiver — under the new rule it
must handle both routes:

```rust
#[objectspell::receiver]
impl Connector for Starter {
    pub async fn connected(&self) {
        self.started().await;
    }

    pub async fn disconnected(&self) {}
}
```

Create `objectspell/tests/ordering.rs`, moving `Producer`, `Consumer`, their receivers, and
`work_queued_before_disconnect_is_still_handled`. Add the same no-op to `Producer`'s Connector
receiver:

```rust
#[objectspell::receiver]
impl Connector for Producer {
    pub async fn connected(&self) {
        for value in 0..5 {
            self.item(value.to_string()).await;
        }
        self.done().await;
    }

    pub async fn disconnected(&self) {}
}
```

Both new files need the `Log`/`log()`/`entries()` helpers and a `run()` that takes the new return
type:

```rust
async fn run(topology: impl std::future::Future<Output = Result<(), objectspell::WiringError>>) {
    tokio::time::timeout(Duration::from_secs(10), topology)
        .await
        .expect("the topology shut down on its own")
        .expect("the topology wiring is valid");
}
```

- [ ] **Step 6: Run everything**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy clean. If any example fails to compile, a receiver in it is missing a
route — add the no-op handler the error names.

- [ ] **Step 7: Verify both examples still run**

```bash
cargo build -p simple -p pubsub
perl -e 'alarm 20; exec @ARGV' -- ./target/debug/simple; echo "exit=$?"
perl -e 'alarm 40; exec @ARGV' -- ./target/debug/pubsub; echo "exit=$?"
```

Expected: both print their usual output and `exit=0`. (`timeout` is not installed on this
machine; `perl -e 'alarm N'` is the substitute.)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Check a topology before starting it"
```

---

### Task 5: The proposal for optional no-op handlers

**Files:**
- Create: `~/lab/objectspell-rs/docs/proposals/optional-no-op-handlers.md`
- Create: `~/lab/objectspell-py/docs/proposals/optional-no-op-handlers.md`

**Interfaces:** none — documentation only.

**Why:** Task 4 forced two no-op `disconnected` handlers into the test suite, and
`examples/pubsub` already carries three more (`App::weather_determined`,
`App::something_happened`, `Connector::started`). The rule is the right default, but the ceremony
it creates is worth removing later.

- [ ] **Step 1: Write the proposal**

Both files get the same content. Write it self-contained — neither repo may reference the other
(see Global Constraints), so state the problem in terms of the library it sits in.

```markdown
# Proposal: let a receiver omit handlers it would leave empty

## Status

Proposed. Not scheduled. Needs to land in both implementations to keep them aligned.

## Problem

A receiver must define one method per signal its channel declares, and `connect()` rejects a
topology where one is missing. The rule is a good default: silence about a signal you receive is
usually an oversight, and finding it at startup beats finding it when the signal first fires.

But a component often genuinely cares about only part of a channel. Today it must still write
the other handlers as empty bodies. In this repository's own `pubsub` example, three handlers
exist for no reason other than the rule, and the shutdown tests add two more.

An empty body is indistinguishable from a handler someone meant to fill in. The rule that was
supposed to catch oversights ends up manufacturing things that look exactly like oversights.

## Proposed change

Let a receiver declare that it deliberately ignores the rest of its channel, and check only the
routes it did not opt out of. Sketch of the two directions worth exploring:

1. **Opt out per receiver.** A marker on the receiver block meaning "the routes I do not name
   are deliberately ignored". Keeps the default strict; makes the exception explicit and
   greppable.
2. **Opt out per route.** Name the ignored routes explicitly, so adding a new signal to a channel
   still fails validation for every receiver that has not considered it. Stricter, more typing.

(1) is less ceremony; (2) preserves the property that a *new* signal is never silently ignored.
(2) is probably the right trade, since that property is most of the rule's value.

## Not in scope

Relaxing the check for a receiver that names a channel nothing emits. That is a different
mistake and should stay an error.

## Open questions

- Does the opt-out belong on the receiver block, or on the component?
- Should a receiver that opts out of *every* route be an error? It listens to a channel and
  handles nothing, which is almost certainly a mistake.
```

- [ ] **Step 2: Verify the constraint**

```bash
cd ~/lab/objectspell-rs && grep -rniE "python|objectspell-py" docs/
cd ~/lab/objectspell-py && grep -rniE "objectspell-rs|rust" docs/proposals/
```

Expected: no output from either.

- [ ] **Step 3: Commit, in both repos**

```bash
cd ~/lab/objectspell-rs && git add docs && git commit -m "Propose optional no-op handlers in receivers"
cd ~/lab/objectspell-py && git add docs && git commit -m "Propose optional no-op handlers in receivers"
```

---

## Verification

From `~/lab/objectspell-rs`, after all tasks:

```bash
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo test --workspace                                  # all tests pass
cargo build -p simple -p pubsub
perl -e 'alarm 20; exec @ARGV' -- ./target/debug/simple   # exit 0, unchanged output
perl -e 'alarm 40; exec @ARGV' -- ./target/debug/pubsub    # exit 0, unchanged output
grep -rniE "python|objectspell-py" --include="*.rs" --include="*.md" --include="*.toml" . \
  | grep -v "^./target"                                  # no output
```

Then check the errors read well from a user's seat — temporarily break
`examples/simple/src/y.rs` by renaming its `something_happened` handler to `somethin_happened`,
run `cargo run -p simple`, confirm the message names the component, the channel, and the missing
route, and revert.

## Finishing

Update `~/.claude/plans/analyze-lab-objectspell-rs-library-it-s-playful-cray.md`: set the **2c**
row to done, and note in Phase 3 that the test suite is now four binaries and why. Then use
superpowers:finishing-a-development-branch.
