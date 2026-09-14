# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

First release. Nothing is published yet, so this section becomes `0.1.0` when it ships.

### Added

- **The four pieces**: `#[objectspell::emitter]` declares what a component sends,
  `#[objectspell::state]` makes a struct a component, `#[objectspell::receiver]` declares what
  it listens to, and `Connector` starts and stops a topology.
- **Wiring by type name.** A receiver block reads `impl Sender for Listener` — no registries, no
  callback lists, and no component referring to another's types.
- **Connect-time validation.** `connect()` returns `Result<(), WiringError>` and checks the whole
  topology before linking or starting anything. It reports a receiver named after a component
  that is not connected, a receiver that misses one of its channel's signals, and two connected
  components sharing a name.
- **Graceful shutdown.** `Connector::disconnect()` emits `disconnected`; every listening
  component handles it, so one signal stops the topology and `connect()` returns. Signals queued
  before the shutdown are still handled, and a handler already running always finishes.
- **Typed signal arguments.** Arguments travel as values behind an `Arc`, matched by parameter
  name and read back by type. A mismatch between an emitter and a handler panics with a message
  naming the channel, the route, and the argument.
- **Handlers may take `&mut self`.** A component handles one signal at a time behind an exclusive
  lock, so its own fields need no lock of yours.

### Notes

- Requires Rust 1.82.
- Four dependencies: `objectspell-macros`, `tokio`, `async-trait`, `inventory`.
- Signal declarations take no visibility, the same as real trait methods, and the generated
  senders are public. Writing `pub` is still accepted.
