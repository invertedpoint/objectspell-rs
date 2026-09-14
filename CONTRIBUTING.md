# Contributing

Thanks for taking a look.

## Setup

```bash
git clone https://github.com/invertedpoint/objectspell-rs
cd objectspell-rs
cargo build --workspace
```

Rust 1.82 or newer. Nothing else to install.

## Checks

Run these before opening a pull request:

```bash
cargo fmt --all --check                                  # formatting
cargo clippy --workspace --all-targets -- -D warnings    # lints, warnings are errors
cargo test --workspace                                   # tests, including doctests
cargo doc --no-deps --workspace                          # rustdoc, no warnings
```

Both library crates carry `#![warn(missing_docs)]`, so an undocumented public item fails the
clippy step. `cargo test --workspace` also compiles and runs the README's quick start, so a
change to the API that the README demonstrates will fail there rather than rotting quietly.

And check the examples still run and still exit on their own:

```bash
cargo run -p simple
cargo run -p pubsub
```

## Writing tests

Receivers register themselves through [`inventory`](https://docs.rs/inventory), which collects
per **test binary** — that is, per file in `objectspell/tests/`. Components in one file are
invisible to another, but every topology *within* a file shares the same registrations.

The rule that follows: **every channel the `Connector` receives must be present in every topology
in that file.** If one test adds `impl Producer for Connector`, then every other `connect()` call
in that file must include a `Producer`, or validation rejects it with `UnknownChannel`.

When a new test needs a `Connector` receiver that the existing ones do not, add a **new file**.
Do not reshape the existing topologies to accommodate it. That is why the suite is split the way
it is:

| File | What it covers | `Connector` receivers |
|---|---|---|
| `wiring.rs` | registration and declared routes | none — never calls `connect()` |
| `shutdown.rs` | stopping a topology | one |
| `ordering.rs` | work queued before shutdown | one |
| `validation.rs` | the three wiring errors | none, though it does call `connect()` |
| `macros.rs` | what the macros generate | none |
| `lints.rs` | the macros emit nothing undocumented | none |

Three more things worth knowing:

- Components must be declared at **module scope**, not inside a test function. The `receiver`
  macro expands to an `inventory` submission, which needs item position.
- Prefer testing `wiring::validate` directly over building a topology. It is a plain async
  function over `&[&dyn AnyState]`, so a test of a *broken* topology needs no shutdown path and
  no `Connector` receiver at all.
- Wrap any topology that calls `connect()` in `tokio::time::timeout`. A shutdown bug otherwise
  hangs the suite instead of failing it.

## Notes

- Keep the dependency count at four. `Display` and `Error` are written by hand rather than
  pulling in a derive crate, and that is a deliberate trade.
- Public behaviour changes should come with a documentation update in `docs/`, and a line in
  `CHANGELOG.md`.
- Ideas that need to land in more than one place live in `docs/proposals/`.
