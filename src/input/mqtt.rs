// use std::sync::Arc;

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

use super::InputSourceHandler;
use crate::config::InputSource;
use crate::message::Message;

pub struct MqttInputSource {
    name: String,
    topic: String,
}

impl MqttInputSource {
    pub fn new(name: &str, source: &InputSource) -> Result<Self, String> {
        let topic = source.topic.clone();

        // Extract the MQTT client from the input source (client ID, QoS, etc.)

        Ok(Self {
            name: name.to_string(),
            topic,
        })
    }
}

#[async_trait]
impl InputSourceHandler for MqttInputSource {
    async fn run(
        &self,
        tx: Sender<Message>,
        mut shutdown: Receiver<()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Starting MQTT input source [{}] on topic '{}'",
            self.name, self.topic
        );

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

        let message = Message {
            source: self.name.clone(),
            topic: "simulated".into(),
            payload: serde_json::json!({ "text": "liba!!!!" }),
            timestamp: now,
        };

        if let Err(e) = tx.send(message).await {
            error!("Failed to send message from {}: {}", self.name, e);
        }

        info!("MQTT input source [{}] completed", self.name);
        Ok(())
    }
}
