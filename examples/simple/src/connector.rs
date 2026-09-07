//! connector

use objectspell::Connector;

#[objectspell::receiver]
impl Y for Connector {
    pub async fn completed(&self) {
        self.disconnected().await;
    }
}
