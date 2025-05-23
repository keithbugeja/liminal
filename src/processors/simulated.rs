use super::processor::Processor;

use crate::config::{StageConfig, extract_param};
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;

pub struct SimulatedInputStage {
    name: String,
    interval_ms: u64,
    counter: u64,
}

impl SimulatedInputStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let interval_ms = extract_param(&config.parameters, "interval_ms", 1000);

        Box::new(Self {
            name: name.to_string(),
            interval_ms,
            counter: 0,
        })
    }
}

#[async_trait]
impl Processor for SimulatedInputStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Simulated input stage [{}] initialized", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        _: &mut HashMap<String, Subscriber<Message>>,
        output: Option<&Arc<dyn PubSubChannel<Message>>>,
    ) -> anyhow::Result<()> {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(self.interval_ms)) => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

                let message = Message {
                    source: self.name.clone(),
                    topic: "simulated".into(),
                    payload: serde_json::json!({ "counter": self.counter }),
                    timestamp: now,
                };

                if let Some(output_channel) = output {
                    let _ = output_channel.publish(message).await;
                }

                self.counter += 1;
            }
        }

        Ok(())
    }
}
