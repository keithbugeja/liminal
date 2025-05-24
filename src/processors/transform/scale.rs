use super::super::processor::Processor;

use crate::config::{StageConfig, extract_param, extract_field_params, FieldConfig};
use crate::core::context::ProcessingContext;
use crate::core::message::Message;

use async_trait::async_trait;
use tokio::select;

pub struct ScaleProcessor {
    name: String,
    scale_factor: f64,
    field_config: FieldConfig,
}

impl ScaleProcessor {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let scale_factor = extract_param(&config.parameters, "scale_factor", 0.5);
        let field_config = extract_field_params(&config.parameters);

        Box::new(Self {
            name: name.to_string(),
            scale_factor,
            field_config,
        })
    }
}

#[async_trait]
impl Processor for ScaleProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Scale filter stage [{}] initialized", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        context: &mut ProcessingContext,
    ) -> anyhow::Result<()> {
        if let Some((name, input)) = context.inputs.iter_mut().next() {
            select! {
                message = input.recv() => {
                    if let Some(message) = message {
                        let mut payload = serde_json::json!({});

                        match &self.field_config {
                            FieldConfig::Single {input, output} => {
                                if let Some(input_value) = message.payload.get(input) {
                                    let scaled_value = input_value.as_f64().unwrap_or(0.0) * self.scale_factor;
                                    payload[output] = serde_json::json!(scaled_value);
                                }
                            }

                            // Scale multiple fields
                            FieldConfig::Multiple { inputs, outputs } => {
                                for (input, output) in inputs.iter().zip(outputs.iter()) {
                                    if let Some(input_value) = message.payload.get(input) {
                                        let scaled_value = input_value.as_f64().unwrap_or(0.0) * self.scale_factor;
                                        payload[output] = serde_json::json!(scaled_value);
                                    }
                                }
                            },
                            _ => {
                                tracing::warn!("Invalid field configuration for scale processor");
                            }
                        }
                        
                        // Apply scaling to the message payload
                        if let Some(output_info) = &context.output {
                            let scaled_message = Message {
                                source: self.name.clone(),
                                topic: output_info.name.clone(),
                                payload,
                                timestamp: message.timestamp,
                            };

                            let _ = output_info.channel.publish(scaled_message).await;
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => { }
            }
        }

        Ok(())
    }
}
