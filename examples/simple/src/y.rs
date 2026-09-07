//! y

#[objectspell::emitter]
pub trait Y {
    pub async fn completed();
}

#[objectspell::state]
pub struct Y {}

impl Y {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn show(&self, message: &str) {
        println!("Y.show: {}", message);
    }
}

#[objectspell::receiver]
impl X for Y {
    pub async fn something_happened(&self, message: String) {
        self.show(&message);
        self.completed().await;
    }
}
