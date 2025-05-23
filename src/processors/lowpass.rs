use super::processor::Processor;
use crate::config::StageConfig;
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub struct LowPassFilterStage {
    name: String,
    threshold: f64,
}

impl LowPassFilterStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let threshold = config
            .parameters
            .as_ref()
            .and_then(|p| p.get("threshold"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        Box::new(Self {
            name: name.to_string(),
            threshold,
        })
    }
}

#[async_trait]
impl Processor for LowPassFilterStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Low pass filter stage [{}] initialized", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        inputs: &mut HashMap<String, Subscriber<Message>>,
        output: Option<&Arc<dyn PubSubChannel<Message>>>,
    ) -> anyhow::Result<()> {
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
