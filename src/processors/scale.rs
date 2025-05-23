use super::processor::Processor;

use crate::config::{StageConfig, extract_param};
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::select;

pub struct ScaleFilterStage {
    name: String,
    scale: f64,
}

impl ScaleFilterStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let scale = extract_param(&config.parameters, "scale_factor", 0.5);

        Box::new(Self {
            name: name.to_string(),
            scale,
        })
    }
}

#[async_trait]
impl Processor for ScaleFilterStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Scale filter stage [{}] initialized", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        inputs: &mut HashMap<String, Subscriber<Message>>,
        output: Option<&Arc<dyn PubSubChannel<Message>>>,
    ) -> anyhow::Result<()> {
        if let Some((name, input)) = inputs.iter_mut().next() {
            select! {
                message = input.recv() => {
                    if let Some(message) = message {
                        println!("Received from [{}]: {:?}", name, message);

                        // Apply scaling to the message payload
                        if let Some(payload) = message.payload.as_f64() {
                            let scaled_value = payload * self.scale;
                            let scaled_message = Message {
                                source: self.name.clone(),
                                topic: message.topic.clone(),
                                payload: serde_json::json!({ "scaled_value": scaled_value }),
                                timestamp: message.timestamp,
                            };

                            if let Some(output_channel) = output {
                                let _ = output_channel.publish(scaled_message).await;
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    tracing::info!("Scale filter stage [{}] processing", self.name);
                }
            }
        }

        for (name, input) in inputs.iter_mut() {
            if let Some(message) = input.recv().await {
                println!("Received from [{}]: {:?}", name, message);

                if let Some(output_channel) = output {
                    let _ = output_channel.publish(message).await;
                }
            }
        }

        Ok(())
    }
}
