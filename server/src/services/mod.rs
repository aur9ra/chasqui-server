pub mod cache;
pub mod sync;

use anyhow::Result;
use async_trait::async_trait;
use chasqui_core::notifier::ContentBuildNotifier;
use reqwest::Client;

pub struct WebhookBuildNotifier {
    client: Client,
    url: String,
    secret: String,
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
        if self.url.is_empty() {
            return Ok(());
        }

        let mut request = self.client.post(&self.url);

        if !self.secret.is_empty() {
            request = request.header("X-Webhook-Secret", &self.secret);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            eprintln!(
                "Webhook notification failed with status: {}",
                response.status()
            );
        }

        Ok(())
    }
}