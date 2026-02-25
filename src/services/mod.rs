use async_trait::async_trait;
use anyhow::Result;
use reqwest::Client;

pub mod sync;

#[async_trait]
pub trait ContentBuildNotifier: Send + Sync {
    async fn notify(&self) -> Result<()>;
}

pub struct WebhookBuildNotifier {
    pub client: Client,
    pub url: String,
    pub secret: String,
}

impl WebhookBuildNotifier {
    pub fn new(url: String, secret: String) -> Self {
        Self {
            client: Client::new(),
            url,
            secret,
        }
    }
}

#[async_trait]
impl ContentBuildNotifier for WebhookBuildNotifier {
    async fn notify(&self) -> Result<()> {
        println!("WebhookBuildNotifier: Triggering build at {}...", self.url);
        let res = self.client
            .post(&self.url)
            .header("Authorization", format!("Bearer {}", self.secret))
            .send()
            .await;

        match res {
            Ok(response) if response.status().is_success() => {
                println!("WebhookBuildNotifier: Success.");
                Ok(())
            }
            Ok(response) => {
                anyhow::bail!("Frontend rejected build request. Status: {}", response.status());
            }
            Err(e) => {
                anyhow::bail!("Failed to connect to frontend webhook: {}", e);
            }
        }
    }
}
