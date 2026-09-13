//! connector

use objectspell::Connector;

#[objectspell::receiver]
impl Y for Connector {
    async fn completed(&self) {
        self.disconnect().await;
    }
}
