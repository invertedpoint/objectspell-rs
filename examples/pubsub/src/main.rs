//! Several components publishing at once.
//!
//! `Weather` and `News` publish on their own schedules, `Monitor` logs everything, and `App`
//! stops the program once both sources have finished.

pub mod app;
pub mod connector;
pub mod monitor;
pub mod news;
pub mod util;
pub mod weather;

use objectspell::Connector;

use app::App;
use monitor::Monitor;
use news::News;
use weather::Weather;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = Connector::new();
    let app = App::new();
    let weather = Weather::new();
    let news = News::new();
    let monitor = Monitor::new();

    connector.connect((app, weather, news, monitor)).await?;

    Ok(())
}
