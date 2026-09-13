use crate::util::rand_index;

#[objectspell::emitter]
pub trait News {
    async fn something_happened(message: String);
    async fn news_completed();
}

#[objectspell::state]
pub struct News {
    pub news_items: Vec<&'static str>,
}

impl News {
    pub fn new() -> Self {
        Self {
            news_items: vec!["Breaking news", "Sports update", "Tech news", "Local news"],
            ..Default::default()
        }
    }
}

#[objectspell::receiver]
impl App for News {
    async fn started(&self) {
        for _ in 0..5 {
            let item = self.news_items[rand_index(self.news_items.len())];
            let msg = format!("{}: Something interesting happened!", item);
            self.something_happened(msg).await;
            // Wait a bit between messages
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        self.news_completed().await;
    }

    async fn stopped(&self) {}
}
