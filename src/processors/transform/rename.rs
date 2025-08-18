use crate::processors::Processor;
use crate::config::{extract_field_params, extract_param, FieldConfig, ProcessorConfig, StageConfig};
use crate::core::context::ProcessingContext;
use crate::core::message::Message;

use anyhow::anyhow;
use async_trait::async_trait;
use serde_json::{Map, Value};
use tokio::select;

#[derive(Debug, Clone)]
struct RenameConfig {
    field_config: FieldConfig,
    drop_original: bool,
}

impl ProcessorConfig for RenameConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let field_config = extract_field_params(&config.parameters);
        if matches!(field_config, FieldConfig::None) {
            return Err(anyhow!("rename requires a field mapping"));
        }
        let drop_original = extract_param(&config.parameters, "drop_original", true);
        Ok(Self { field_config, drop_original })
    }

    fn validate(&self) -> anyhow::Result<()> {
        self.field_config.validate()
    }
}

pub struct RenameProcessor {
    name: String,
    config: RenameConfig,
}

impl RenameProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let rename_config = RenameConfig::from_stage_config(&config)?;
        rename_config.validate()?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: rename_config,
        }))
    }

    fn transform_payload(&self, payload: &Value) -> Value {
        let Some(obj) = payload.as_object() else { return payload.clone(); };
        let mut result = if self.config.drop_original { Map::new() } else { obj.clone() };

        // Use FieldConfig's apply method if it exists, or implement based on the actual variants
        match &self.config.field_config {
            FieldConfig::Single { input, output } => {
                if let Some(value) = obj.get(input) {
                    result.insert(output.clone(), value.clone());
                }
            }
            FieldConfig::Multiple { inputs, outputs } => {
                for (input_field, output_field) in inputs.iter().zip(outputs.iter()) {
                    if let Some(value) = obj.get(input_field) {
                        result.insert(output_field.clone(), value.clone());
                    }
                }
            }
            FieldConfig::Mapping(map) => {
                for (input_field, output_field) in map {
                    if let Some(value) = obj.get(input_field) {
                        result.insert(output_field.clone(), value.clone());
                    }
                }
            }
            _ => {}
        }

        Value::Object(result)
    }
}

#[async_trait]
impl Processor for RenameProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!(
            "Rename processor '{}' initialised (drop_original={}, fields={:?})",
            self.name, self.config.drop_original, self.config.field_config
        );
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        if let Some((_, input)) = context.inputs.iter_mut().next() {
            select! {
                message = input.recv() => {
                    if let Some(message) = message {
                        let transformed_payload = self.transform_payload(&message.payload);

                        if let Some(output_info) = &context.output {
                            let output_message = Message {
                                source: self.name.clone(),
                                topic: output_info.name.clone(),
                                payload: transformed_payload,
                                timestamp: message.timestamp,
                            };

                            tracing::debug!(
                                "Renaming message from '{}' to '{}': {:?}",
                                message.topic,
                                output_info.name,
                                output_message
                            );

                            if let Err(e) = output_info.channel.publish(output_message).await {
                                tracing::warn!("Failed to publish renamed message: {:?}", e);
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
            }
        }
        Ok(())
    }
}