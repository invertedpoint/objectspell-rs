#[objectspell::emitter]
pub trait Monitor {}

#[objectspell::state]
pub struct Monitor {}

impl Monitor {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn show(&self, text: &str) {
        println!("[Monitor] {}", text);
    }
}

#[objectspell::receiver]
impl App for Monitor {
    pub async fn started(&self) {
        self.show("App started");
    }

    pub async fn stopped(&self) {
        self.show("App stopped");
    }
}

#[objectspell::receiver]
impl Weather for Monitor {
    pub async fn weather_determined(&self, message: String) {
        self.show(&format!("Weather update: {}", message));
    }

    pub async fn weather_completed(&self) {
        self.show("All weather updates completed");
    }
}

#[objectspell::receiver]
impl News for Monitor {
    pub async fn something_happened(&self, message: String) {
        self.show(&format!("News update: {}", message));
    }

    pub async fn news_completed(&self) {
        self.show("All news updates completed");
    }
}
