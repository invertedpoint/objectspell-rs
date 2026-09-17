# Proposal: report runtime failures instead of printing or panicking

## Status

Proposed. Not scheduled. Breaking — would ship as 0.2.0.

## Problem

A component's listener runs in a detached task with no way to report failure. Lacking a return
path, the library improvises differently at each site where something can go wrong:

| Where | What it does | What the caller sees |
|---|---|---|
| `state.rs`, unknown channel | `eprintln!`, continues | a line on stderr, signal dropped |
| `state.rs`, unhandled route | `eprintln!`, continues | the same |
| `signal.rs`, `expect_param` | panics inside the task | the component dies |
| `connector.rs`, `let _ = handle.await` | discards the result | nothing at all |

These are one defect wearing three costumes. A library should not write to stderr, and should not
panic on data a caller could legitimately produce.

The third and fourth rows compound into a real failure: a panicking handler kills its component
silently, and if that component was the one that would have called `disconnect()`, the program
hangs with no output. This is not hypothetical — `examples/pubsub` hit it when a handler
parameter name did not match the signal's.

Connect-time validation removed most *reachable* paths to the first two rows, but they remain
reachable through `EmitterCore::broadcast` and `StateCore`, both public.

## Proposed change

Give the listener a return path, and let `connect()` collect what comes back.

1. **`SignalDispatcher::dispatch` returns a `Result`.** Its only failure is reading an argument,
   so `Signal` gains `try_param` returning `Result<T, ArgumentError>` next to the existing
   `param` and `expect_param`. Generated code uses `try_param`; `expect_param` stays as a
   documented-panic convenience for direct callers, which is ordinary Rust.

2. **`listen` returns `Result<(), RuntimeError>`.** Both `eprintln!` sites become returns, and the
   loop stops at the first error rather than continuing — a component that received something it
   cannot handle is in a state the library cannot reason about. `RuntimeError` names the
   component, and covers an unknown channel, an unhandled route, an argument that could not be
   read, and a listener that panicked.

3. **`connect()` returns every failure.** `Result<(), Error>` where `Error` is either a
   `WiringError` as today or a `Vec<RuntimeError>` — usually empty or a single entry, but honest
   when two components fail for related reasons.

4. **A failing listener winds the topology down.** Collecting errors is not enough on its own: if
   one listener stops and nothing calls `disconnect()`, `connect()` still waits on the others and
   the program hangs, now with better diagnostics. Driving the listeners through a
   `tokio::task::JoinSet` lets `connect()` see the first failure as it happens and emit
   `disconnected`, so the topology ends and the error is returned. This is the part that turns the
   defect from *reported* into *fixed*.

## A related gap

Validation checks channel and route **names**, not parameter names. A handler declaring `mesage`
where the signal declares `message` compiles, passes `connect()`, and fails at the first signal —
exactly the failure above, with a cause that was knowable at startup.

The `EmitterRegistration` that validation already relies on could carry each signal's parameter
names, and dispatchers could report the ones their handler declares. The check would be a subset,
not an equality: a handler may legitimately ignore parameters it does not need, and a typo fails
because the misspelling is not among the ones declared.

Worth doing in the same change — it shrinks what can still go wrong at run time — but it stands on
its own.

## Not in scope

- **Backpressure.** Queues stay unbounded; a producer outrunning its consumer still grows memory.
  A separate concern with a separate design.
- **Retries or a dead-letter queue.** A failed signal stays failed. Handle expected failures
  inside handlers.
- **Catching panics inside `dispatch`.** The listener task already carries a panic to its join
  handle; `connect()` only has to stop discarding it. No `catch_unwind`.

## Open questions

- Should a handler's own panic stop only its component, or always end the topology? Stopping one
  component is more predictable; ending everything is easier to notice. The proposal above ends
  everything, on the grounds that a half-dead topology is the worse failure.
- Is stopping the listener at the first error right, or should an unhandled route be skipped and
  reported without ending the component? The two rows differ in severity and could reasonably
  differ in treatment.
- `Error::Runtime(Vec<RuntimeError>)` is honest but awkward to match on. An alternative is the
  first error plus a count.
