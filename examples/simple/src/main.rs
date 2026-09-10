//! The smallest complete topology.
//!
//! - `X` emits `something_happened`
//! - `Y` receives it, shows the message, and emits `completed`
//! - `Connector` starts everything and stops it once `Y` is done

pub mod connector;
pub mod x;
pub mod y;

use objectspell::Connector;

use x::X;
use y::Y;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = Connector::new();
    let x = X::new();
    let y = Y::new();

    connector.connect((x, y)).await?;

    Ok(())
}
