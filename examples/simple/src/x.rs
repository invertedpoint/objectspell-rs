//! x

#[objectspell::emitter]
pub trait X {
    async fn something_happened(message: String);
}

#[objectspell::state]
pub struct X {}

impl X {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub async fn action(&self) {
        println!("Doing something...");
        self.something_happened("OK".to_string()).await;
    }
}

#[objectspell::receiver]
impl Connector for X {
    async fn connected(&self) {
        println!("X.Connector.connected");
        self.action().await;
    }

    async fn disconnected(&self) {
        println!("X.Connector.disconnected");
    }
}
