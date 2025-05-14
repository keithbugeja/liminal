use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::{Duration, sleep};
use tracing::{error, info};

use super::InputSourceHandler;
use crate::config::InputSource;
use crate::core::message::Message;

pub struct SimulatedInputSource {
    name: String,
    interval_ms: u64,
}

impl SimulatedInputSource {
    pub fn new(name: &str, source: &InputSource) -> Result<Self, String> {
        let interval_ms = source
            .parameters
            .as_ref()
            .and_then(|p| p.get("interval_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Ok(Self {
            name: name.to_string(),
            interval_ms,
        })
    }
}

#[async_trait]
impl InputSourceHandler for SimulatedInputSource {
    async fn run(
        &self,
        tx: Sender<Message>,
        mut shutdown: Receiver<()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Starting simulated input [{}] at {}ms intervals",
            self.name, self.interval_ms
        );

        let mut counter = 0x64;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(self.interval_ms)) => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

                    let message = Message {
                        source: self.name.clone(),
                        topic: "simulated".into(),
                        payload: serde_json::json!({ "counter": counter }),
                        timestamp: now,
                    };

                    if let Err(e) = tx.send(message).await {
                        error!("Failed to send message from {}: {}", self.name, e);
                    }

                    // info!(target: "simulated", "{} => simulated message {}", self.name, counter);
                    counter += 1;
                }
                _ = shutdown.recv() => {
                    info!("Simulated input [{}] received shutdown signal.", self.name);
                    break;
                }
            }
        }

        Ok(())
    }
}
