use objectspell::Connector;

#[objectspell::receiver]
impl App for Connector {
    pub async fn started(&self) {}

    pub async fn stopped(&self) {
        self.disconnect().await;
    }
}
