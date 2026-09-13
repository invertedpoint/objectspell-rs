#[objectspell::emitter]
pub trait App {
    async fn started();
    async fn stopped();
}

#[objectspell::state]
pub struct App {
    pub uncompleted_sources: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            uncompleted_sources: vec!["Weather".to_string(), "News".to_string()],
            ..Default::default()
        }
    }

    pub async fn use_completion(&mut self, completed_source: &str) {
        self.uncompleted_sources
            .retain(|source| source != completed_source);

        if self.uncompleted_sources.is_empty() {
            self.stopped().await;
        }
    }
}

#[objectspell::receiver]
impl Connector for App {
    async fn connected(&self) {
        self.started().await;
    }

    async fn disconnected(&self) {}
}

#[objectspell::receiver]
impl Weather for App {
    async fn weather_determined(&self, _message: String) {}

    async fn weather_completed(&mut self) {
        self.use_completion("Weather").await;
    }
}

#[objectspell::receiver]
impl News for App {
    async fn something_happened(&self, _message: String) {}

    async fn news_completed(&mut self) {
        self.use_completion("News").await;
    }
}
