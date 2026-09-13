# Limitations

Worth knowing before you build on this.

## One topology per binary

Receivers register themselves through [`inventory`](https://docs.rs/inventory), which collects
per **binary**, not per topology. Every `#[objectspell::receiver]` compiled into your program
counts, whether or not you meant to use it now.

In practice:

- **Component type names must be unique** across everything you compile in. `a::Worker` and
  `b::Worker` are the same channel, and `connect()` rejects a topology containing both.
- **Every receiver you write must have its component connected.** You cannot write handlers for
  an optional component and then leave it out of the tuple; `connect()` rejects that instead.
- **Two topologies in one binary must be compatible.** This bites hardest in tests: each
  `tests/*.rs` file is its own binary, so components in one file are invisible to another, but
  every topology *within* a file shares the same registrations. If one test adds a receiver on
  the `Connector`, every other topology in that file must contain the component it names. The
  fix is a new test file, not a cleverer topology. This crate's own suite is split that way.

## Wiring is fixed once connected

Components are linked during `connect()` and stay linked. There is no way to add a component,
remove one, or unsubscribe a handler while running.

## Signal arguments must be `Clone + Send + Sync + 'static`

A signal is broadcast to every connected receiver, so each argument is held behind an `Arc` and
cloned out on the receiving side. Types that are not `Clone`, or that borrow, cannot travel in a
signal — send an owned summary instead.

Arguments are matched by parameter name and read back by type. A mismatch between an emitter's
declaration and a handler's is caught at run time, by a panic naming both, rather than by the
compiler.

## One handler at a time, per component

A component handles signals in sequence behind an exclusive lock. That is what makes `&mut self`
handlers safe without a lock of your own, but it also means a slow handler blocks everything else
queued for that component. Long or blocking work belongs on its own component, or on a task the
handler spawns.

## Single process, in memory

Signals are values passed between unbounded Tokio channels in one process. Nothing crosses a
process or a network, nothing is persisted, and nothing survives a restart. If a component's
listener stops, signals queued for it are lost.

Queues are unbounded, so there is no backpressure: a producer faster than its consumer grows
memory rather than slowing down.

This is a wiring library, not a message broker. If you need delivery guarantees, durability, or
multiple machines, use a broker and let ObjectSpell organise the components around it.

## Signals are one-way

A signal has no return value and no reply. To send something back, declare a signal in the other
direction.

## A panicking handler is not reported

It ends that component's listener without failing `connect()`. See
[Lifecycle](lifecycle.md#errors).
