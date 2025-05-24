use crate::processors::Processor;

use crate::config::{StageConfig, extract_param, extract_field_params, FieldConfig};
use crate::core::context::ProcessingContext;
use crate::core::message::Message;
use crate::config::ProcessorConfig;

use async_trait::async_trait;
use tokio::select;

#[derive(Debug, Clone)]
pub struct ScaleConfig {
    pub scale_factor: f64,
    pub field_config: FieldConfig,
}

impl ProcessorConfig for ScaleConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let scale_factor = extract_param(&config.parameters, "scale_factor", 1.0);
        let field_config = extract_field_params(&config.parameters);

        Ok(Self {
            scale_factor,
            field_config,
        })
    }
}

pub struct ScaleProcessor {
    name: String,
    config: ScaleConfig,
}

impl ScaleProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let scale_config = ScaleConfig::from_stage_config(&config)?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: scale_config,
        }))
    }
}

#[async_trait]
impl Processor for ScaleProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Scale processor [{}] initialised", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        context: &mut ProcessingContext,
    ) -> anyhow::Result<()> {
        if let Some((_, input)) = context.inputs.iter_mut().next() {
            select! {
                message = input.recv() => {
                    if let Some(message) = message {
                        let mut payload = serde_json::json!({});

                        match &self.config.field_config {
                            FieldConfig::Single {input, output} => {
                                if let Some(input_value) = message.payload.get(input) {
                                    let scaled_value = input_value.as_f64().unwrap_or(0.0) * self.config.scale_factor;
                                    payload[output] = serde_json::json!(scaled_value);
                                }
                            }

                            // Scale multiple fields
                            FieldConfig::Multiple { inputs, outputs } => {
                                for (input, output) in inputs.iter().zip(outputs.iter()) {
                                    if let Some(input_value) = message.payload.get(input) {
                                        let scaled_value = input_value.as_f64().unwrap_or(0.0) * self.config.scale_factor;
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
