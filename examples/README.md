# Examples

Each is a crate in the workspace, so run them from anywhere in the repository:

```bash
cargo run -p simple
cargo run -p pubsub
```

**`simple/`** — the smallest complete topology. `X` sends one message, `Y` shows it, and the
program stops once `Y` reports it is done. Start here.

**`pubsub/`** — several components at once. `Weather` and `News` publish updates on their own
schedules, `Monitor` logs everything, and `App` shuts the program down once both sources have
finished.

Both use the usual layout: one module per component, holding the component, its emitter block,
and the receivers that listen to it. No component refers to another's types — only `main.rs`
knows the full list.

Both exit on their own. Neither calls `std::process::exit`: the last thing that happens is a
`disconnect()`, which stops every listener and lets `connect()` return.
