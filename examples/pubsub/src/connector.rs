use objectspell::Connector;

#[objectspell::receiver]
impl App for Connector {
    async fn started(&self) {}

    async fn stopped(&self) {
        self.disconnect().await;
    }
}
