#[objectspell::emitter]
pub trait App {
    pub async fn started();
    pub async fn stopped();
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
    pub async fn connected(&self) {
        self.started().await;
    }

    pub async fn disconnected(&self) {
        std::process::exit(0);
    }
}

#[objectspell::receiver]
impl Weather for App {
    pub async fn weather_determined(&self, _message: String) {}

    pub async fn weather_completed(&mut self) {
        self.use_completion("Weather").await;
    }
}

#[objectspell::receiver]
impl News for App {
    pub async fn something_happened(&self, _message: String) {}

    pub async fn news_completed(&mut self) {
        self.use_completion("News").await;
    }
}
