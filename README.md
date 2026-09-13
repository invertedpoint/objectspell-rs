# ObjectSpell

[![crates.io](https://img.shields.io/crates/v/objectspell.svg)](https://crates.io/crates/objectspell)
[![docs.rs](https://docs.rs/objectspell/badge.svg)](https://docs.rs/objectspell)
[![license](https://img.shields.io/crates/l/objectspell.svg)](LICENSE)

Connect async components with signals, without registries, trait objects you have to hand out,
or callback lists. You declare what a component sends and what it listens to; ObjectSpell
connects them by type name.

Built on Tokio. Four dependencies.

## Install

```toml
[dependencies]
objectspell = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

`X` sends a message, `Y` prints it, and the program stops once `Y` is done. Each component is
its own module, holding the component and the receivers that listen to it.

```rust
use objectspell::Connector;

mod x {
    /// Sends one message, as soon as everything is connected.
    #[objectspell::emitter]
    pub trait X {
        /// Emitted once there is something to report.
        pub async fn something_happened(message: String);
    }

    #[objectspell::state]
    pub struct X {}

    #[objectspell::receiver]
    impl Connector for X {
        pub async fn connected(&self) {
            self.something_happened("OK Computer".to_string()).await;
        }

        pub async fn disconnected(&self) {}
    }
}

mod y {
    /// Shows what it is told, then reports that it is done.
    #[objectspell::emitter]
    pub trait Y {
        /// Emitted once the message has been shown.
        pub async fn completed();
    }

    #[objectspell::state]
    pub struct Y {}

    #[objectspell::receiver]
    impl X for Y {
        pub async fn something_happened(&self, message: String) {
            println!("Y received: {message}");
            self.completed().await;
        }
    }
}

mod connector {
    use objectspell::Connector;

    /// Stops everything once `Y` is done.
    #[objectspell::receiver]
    impl Y for Connector {
        pub async fn completed(&self) {
            self.disconnect().await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Connector::new()
        .connect((x::X::default(), y::Y::default()))
        .await?;

    Ok(())
}
```

```console
$ cargo run
Y received: OK Computer
```

Notice that `x` and `y` never refer to each other's types. The only thing linking them is the
name: `y` declares `impl X for Y`, which means *"on `Y`, handle signals from `X`"*. The trait
position names the component you are listening **to**.

The same program split across real files is in [`examples/simple`](examples/simple).

## The four pieces

- **Emitter** — `#[objectspell::emitter]` on a `trait` block declares the signals a component
  can send. The methods have no bodies; they are declarations, and ObjectSpell generates the
  senders.
- **State** — `#[objectspell::state]` on a struct makes it a component. It owns a queue and
  handles one signal at a time, so its own fields need no lock.
- **Receiver** — `#[objectspell::receiver]` on an impl block, **named after the component it
  listens to**.
- **Connector** — starts and stops the whole topology.

## Documentation

- [Concepts](docs/concepts.md) — the four pieces and how a signal travels
- [Wiring](docs/wiring.md) — the naming rule, and what happens when it is wrong
- [Lifecycle](docs/lifecycle.md) — starting up, shutting down, and message order
- [Limitations](docs/limitations.md) — what this library does not do

Runnable examples live in [`examples/`](examples).

## License

MIT — see [LICENSE](LICENSE).
