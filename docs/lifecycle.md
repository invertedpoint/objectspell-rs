# Lifecycle

## Starting

```rust
Connector::new().connect((X::default(), Y::default())).await?;
```

`connect` does five things, in order:

1. **Checks the wiring.** Every receiver must name a connected component and handle all of its
   signals, and no two components may share a name. A problem returns here, before anything is
   linked or started. See [Wiring](wiring.md).
2. **Collects each component's sender** and the channels it listens to.
3. **Links senders to emitters** by name.
4. **Starts a listener** for every component that listens to something. A component with no
   receivers gets none — it has nothing to do and nothing to stop.
5. **Emits `connected`.**

`connect()` does not return until the topology shuts down. It is normally the whole program.

## The `connected` signal

`connected` is the entry point. Listen for it on whichever component should act first:

```rust
// in x.rs
#[objectspell::receiver]
impl Connector for X {
    pub async fn connected(&self) {
        self.action().await;
    }

    pub async fn disconnected(&self) {}
}
```

Both methods are required, because the Connector declares both signals — `disconnected` may do
nothing.

Every component that listens to anything is given a built-in `Connector` receiver, so it is
stopped by `disconnected` whether or not you wrote a handler for it. Writing your own does not
displace the built-in: both run, yours first.

## Stopping

Any component can end the program by calling `disconnect()` on the Connector:

```rust
// in connector.rs
#[objectspell::receiver]
impl Y for Connector {
    pub async fn completed(&self) {
        self.disconnect().await;
    }
}
```

`disconnect()` emits `disconnected`. Every listening component handles it, so that one signal
stops all of them. `connect()` then waits for every listener to finish and returns `Ok(())`.

There is no timeout and no forced stop. If nothing ever calls `disconnect()`, the program runs
forever — the normal case for a long-running service.

## Message order

Each component has its own queue and handles one signal at a time, in arrival order. Handlers on
the same component never overlap, so its fields need no lock — and a handler may take
`&mut self`.

Across components there is no global order. If `X` signals `Y` and `Z`, both receive it, but
which finishes first is undefined.

Because ordering is per queue, work already queued is handled before a shutdown signal queued
after it. The stop flag is read *between* signals, never during one, so a handler that is already
running always finishes. Components complete their pending work as the topology winds down.

## Errors

Two kinds, and they behave very differently.

**Wiring errors** are returned from `connect()` as a `WiringError`, before anything starts. They
are recoverable in the sense that nothing has happened yet.

**A panic inside a handler** ends that component's listener. The panic is reported on stderr, but
`connect()` does not currently surface it: the remaining components keep running, and if the one
that died was the one that would have called `disconnect()`, the topology never shuts down.

So handle expected failures inside your handlers. There is no retry and no dead-letter queue, and
a panicking handler is closer to a silent partial outage than to a crash.
