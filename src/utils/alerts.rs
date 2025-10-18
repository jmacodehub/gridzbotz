//! Smart Alerts - Notify on important events

use reqwest::Client;
use serde_json::json;

pub struct AlertManager {
    webhook_url: Option<String>,
    enabled: bool,
}

impl AlertManager {
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            enabled: webhook_url.is_some(),
            webhook_url,
        }
    }
    
    pub async fn send_test_complete(&self, test_name: &str, roi: f64, duration_min: f64) {
        if !self.enabled {
            return;
        }
        
        let message = format!(
            "🎉 *Test Complete!*\n• Name: {}\n• ROI: {:.2}%\n• Duration: {:.1}m",
            test_name, roi, duration_min
        );
        
        let _ = self.send_webhook(&message).await;
    }
    
    pub async fn send_error(&self, error: &str) {
        if !self.enabled {
            return;
        }
        
        let message = format!("🚨 *Error Detected!*\n``````", error);
        let _ = self.send_webhook(&message).await;
    }
    
    async fn send_webhook(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(url) = &self.webhook_url {
            let client = Client::new();
            client.post(url)
                .json(&json!({"text": message}))
                .send()
                .await?;
        }
        Ok(())
    }
}
