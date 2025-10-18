//! Telegram Bot Integration

use reqwest::Client;
use serde_json::json;

pub struct TelegramBot {
    token: String,
    chat_id: String,
    enabled: bool,
}

impl TelegramBot {
    pub fn new(token: Option<String>, chat_id: Option<String>) -> Self {
        let enabled = token.is_some() && chat_id.is_some();
        Self {
            token: token.unwrap_or_default(),
            chat_id: chat_id.unwrap_or_default(),
            enabled,
        }
    }
    
    pub async fn send_test_complete(&self, name: &str, roi: f64, duration: f64) {
        if !self.enabled { return; }
        let msg = format!("🎉 *Test Complete*\n\nStrategy: `{}`\nROI: *{:.2}%*\nDuration: {:.1}m", 
            name, roi, duration);
        let _ = self.send(&msg).await;
    }
    
    pub async fn send_alert(&self, msg: &str) {
        if !self.enabled { return; }
        let _ = self.send(&format!("🚨 *Alert*\n\n{}", msg)).await;
    }
    
    async fn send(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        Client::new()
            .post(&url)
            .json(&json!({"chat_id": self.chat_id, "text": text, "parse_mode": "Markdown"}))
            .send()
            .await?;
        Ok(())
    }
}
