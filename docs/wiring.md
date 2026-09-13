# Wiring

Components are connected by **name**. This page explains the rule, the layout that suits it, and
what you see when it is wrong.

## The rule

> A receiver block must be named exactly after the component whose signals it handles.

```rust
// in y.rs — Y listens to X
#[objectspell::receiver]
impl X for Y {
    pub async fn something_happened(&self, message: String) {
        self.show(&message);
    }
}
```

Two things follow:

- **The name is the wiring.** `impl X for ...` is what subscribes `Y` to `X`. Change it to
  `impl XSignals for Y` and nothing arrives — the name no longer matches any component.
- **Handler names must match signal names.** `something_happened` here is the same method `X`'s
  emitter block declares, and its parameters must have the same names and types.

A receiver must handle **every** signal its channel declares. If `X` declares
`something_happened` and `finished`, a receiver named `X` must define both, even if one of them
does nothing.

That last rule is deliberate — silence about a signal you receive is usually an oversight — but
it does mean writing empty handlers. Relaxing it is an open
[proposal](proposals/optional-no-op-handlers.md).

## The trait position is a name, not a type

Neither macro resolves the name you write. `#[objectspell::receiver]` takes the channel from the
trait path and then replaces the path entirely; `#[objectspell::emitter]` discards its `trait`
block and generates an inherent `impl`. So `impl X for Y` compiles whether or not anything called
`X` exists — which is exactly why a misspelled name has to be caught at `connect()` rather than
by the compiler.

## Module layout

One component per module — the component, its emitter block, and the receivers that listen to
it — is the layout the examples use, and it scales. It is a convention here, not a requirement:
because the trait position is discarded, a module can hold a component and any number of
receivers without a name clash. `examples/pubsub/src/app.rs` has three receivers alongside the
struct.

```
x.rs          pub struct X                  -> the component
              impl Connector for X          -> X listening to the Connector
y.rs          pub struct Y                  -> the component
              impl X for Y                  -> Y listening to X
connector.rs  impl Y for Connector          -> the Connector listening to Y
main.rs                                     -> the only place that knows the full list
```

Components never refer to each other's types. Only `main.rs` sees them all.

## When wiring is wrong

`connect()` checks the whole topology before linking or starting anything, so mistakes are
reported immediately instead of hanging. It returns `Result<(), WiringError>`; a `main` that uses
`?` prints the message and exits non-zero.

**A name that matches no component:**

```console
$ cargo run
Error: Y declares a receiver named 'Xx', but no connected component emits on that channel. A receiver must be named exactly after the component whose signals it handles. Connected channels: Connector, X, Y.
```

Usually a typo, or a component missing from the tuple passed to `connect()`.

**A receiver that misses a signal:**

```console
$ cargo run
Error: Y.X does not handle every signal on channel 'X': missing something_happened. A receiver must define one method per signal its channel declares.
```

Add the missing method. Note that a misspelled handler produces this same error rather than a
separate one: the typo does not only add an unknown name, it removes a required one.

A hand-written `Connector` receiver needs both `connected` and `disconnected`, even though the
built-in one would have covered the gap.

**Two components with the same name:**

```console
$ cargo run
Error: Two connected components are both named 'Worker'. Routing is by type name, so component names must be unique.
```

Type names, not paths — `a::Worker` and `b::Worker` are the same channel. Rename one.
