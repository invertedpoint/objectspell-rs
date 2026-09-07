use crate::util::rand_index;

#[objectspell::emitter]
pub trait Weather {
    pub async fn weather_determined(message: String);
    pub async fn weather_completed();
}

#[objectspell::state]
pub struct Weather {
    pub weather_conditions: Vec<&'static str>,
}

impl Weather {
    pub fn new() -> Self {
        Self {
            weather_conditions: vec!["Sunny", "Rainy", "Cloudy", "Snowy"],
            ..Default::default()
        }
    }
}

#[objectspell::receiver]
impl App for Weather {
    pub async fn started(&self) {
        for _ in 0..5 {
            let condition = self.weather_conditions[rand_index(self.weather_conditions.len())];
            let msg = format!("Current weather: {}", condition);
            self.weather_determined(msg).await;
            // Wait a bit between messages
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        self.weather_completed().await;
    }

    pub async fn stopped(&self) {}
}
