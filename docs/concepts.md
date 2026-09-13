# Concepts

ObjectSpell has four pieces. Everything else is built from them.

## Emitter

An emitter declares the signals a component can send.

```rust
#[objectspell::emitter]
pub trait X {
    /// Emitted once there is something to report.
    pub async fn something_happened(message: String);
}
```

The methods have no bodies. They are declarations — *this component can send
`something_happened`, and it carries a `message`* — and the macro generates the senders on the
struct of the same name.

The `trait` keyword is borrowed syntax, not a real trait: the block is discarded and replaced
with an inherent `impl X`. Nothing implements it, and you never name it in a bound.

## State

A state is a component. It holds whatever data it needs and handles signals one at a time.

```rust
#[objectspell::state]
pub struct X {}

impl X {
    pub async fn action(&self) {
        self.something_happened("OK Computer".to_string()).await;
    }
}
```

The macro injects the plumbing — a queue, a sender, and the map of receivers — and generates
`init(..)`, `Default`, and `into_state()`.

Each component owns an unbounded queue. Signals wait there and are handled in arrival order
behind an exclusive lock, so a component never runs two handlers at once. That is why a handler
can take `&mut self` and mutate the component's own fields with no lock of your own.

A component with an emitter block can send. One without can only receive. One with no receivers
at all never starts a listener.

## Receiver

A receiver is a group of handlers attached to a component. The **impl block's trait position
says which component it listens to**.

```rust
// Y listens to X
#[objectspell::receiver]
impl X for Y {
    pub async fn something_happened(&self, message: String) {
        println!("Y received: {message}");
    }
}
```

Read it as *on `Y`, handle the signals coming from `X`*. The handler name matches the signal
name, the parameter names match the signal's parameters, and `self` is the `Y`.

Like the emitter block, `X` here is a name, not a trait — it is never resolved as a type. This
naming rule is the core of the library; [Wiring](wiring.md) covers it in full.

## Connector

The Connector starts and stops the topology.

```rust
Connector::new().connect((X::default(), Y::default())).await?;
```

`connect` checks the wiring, links senders to listeners, starts every component, and then emits
`connected`. Calling `disconnect()` emits `disconnected`, which stops everything. See
[Lifecycle](lifecycle.md).

## How a signal travels

Take `self.something_happened("OK Computer".to_string()).await` inside `X`:

1. `X` broadcasts a signal carrying the **channel** `"X"` (the sending component's type name),
   the **route** `"something_happened"` (the method name), and the message
   `{"message": "OK Computer"}`.
2. Every component with a receiver named `X` gets a copy in its queue.
3. Each of those takes it off the queue and calls the handler registered at that channel and
   route.

Dispatch is two map lookups, channel then route — the route is a key, never something a handler
compares itself against.

Arguments travel as values, not as serialised text. Each is stored behind an `Arc` and cloned
out on the receiving side, so a handler parameter must be `Clone + Send + Sync + 'static`. If an
emitter and a handler disagree about a parameter's name or type, the mismatch panics with a
message naming the channel, the route, and the argument — the two declarations that disagree.

Nothing is registered by hand and nothing is looked up by a string you wrote. The channel is a
type name and the route is a method name.
