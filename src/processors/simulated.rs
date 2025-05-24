use super::processor::Processor;

use crate::config::{StageConfig, extract_param};
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;

use async_trait::async_trait;
use rand_distr::{Distribution, Normal, Uniform};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;

pub struct SimulatedInputStage {
    name: String,
    interval_ms: u64,
    distribution: String,
    min_value: f64,
    max_value: f64,
    value_name: String,
}


impl SimulatedInputStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let interval_ms = extract_param(&config.parameters, "interval_ms", 1000);
        let distribution = extract_param(&config.parameters, "distribution", "uniform".to_string());
        let min_value = extract_param(&config.parameters, "min_value", 0.0);
        let max_value = extract_param(&config.parameters, "max_value", 100.0);
        let value_name = extract_param(&config.parameters, "key_out", "value".to_string());

        Box::new(Self {
            name: name.to_string(),
            interval_ms,
            distribution,
            min_value,
            max_value,
            value_name,
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
        // Generate a random value based on the specified distribution
        // and send it to the output channel. The rng is dropped before
        // the select statement to avoid blocking the async runtime.
        let value = {
            let mut rng = rand::rng();
            match self.distribution.as_str() {
                "uniform" => {
                    let uniform = Uniform::new(self.min_value, self.max_value)
                        .unwrap_or_else(|_| Uniform::new(0.0, 1.0).unwrap());
                    uniform.sample(&mut rng)
                }
                "normal" => {
                    let mean = (self.min_value + self.max_value) / 2.0;
                    let stddev = (self.max_value - self.min_value) / 6.0;
                    let normal = Normal::new(mean, stddev)
                        .unwrap_or_else(|_| Normal::new(0.0, 1.0).unwrap());
                    normal.sample(&mut rng).clamp(self.min_value, self.max_value)
                }
                _ => {
                    tracing::warn!("Unknown distribution type: {}, using uniform", self.distribution);
                    let uniform = Uniform::new(self.min_value, self.max_value)
                        .unwrap_or_else(|_| Uniform::new(0.0, 1.0).unwrap());
                    uniform.sample(&mut rng)
                }
            }
        }; 

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(self.interval_ms)) => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

                let message = Message {
                    source: self.name.clone(),
                    topic: "simulated".into(),
                    payload: serde_json::json!({ &self.value_name : value }), // Fix: use value, not 1
                    timestamp: now,
                };

                if let Some(output_channel) = output {
                    let _ = output_channel.publish(message).await;
                }
            }
        }

        Ok(())
    }
}
