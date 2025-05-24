use super::super::processor::Processor;

use crate::config::{StageConfig, extract_param, extract_field_params, FieldConfig};
use crate::core::context::ProcessingContext;
use crate::core::message::Message;
use crate::config::ProcessorConfig;

use async_trait::async_trait;
use tokio::select;

#[derive(Debug, Clone)]
pub struct LowPassConfig {
    pub threshold: f64,
    pub field_config: FieldConfig,
}

impl ProcessorConfig for LowPassConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let threshold = extract_param(&config.parameters, "thresdhold", 25.0);
        let field_config = extract_field_params(&config.parameters);

        Ok(Self {
            threshold,
            field_config,
        })
    }
}

pub struct LowPassProcessor {
    name: String,
    config: LowPassConfig,
}

impl LowPassProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let lowpass_config = LowPassConfig::from_stage_config(&config)?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: lowpass_config,
        }))
    }

   async fn process_message(
        &self,
        message: Message,
        context: &mut ProcessingContext,
    ) -> anyhow::Result<()> {
        let FieldConfig::Single { input, output } = &self.config.field_config else {
            tracing::warn!("Invalid field configuration for lowpass processor");
            return Ok(());
        };

        let Some(input_value) = message.payload.get(input) else {
            return Ok(());
        };

        let value = input_value.as_f64().unwrap_or(0.0);
        if value >= self.config.threshold {
            return Ok(());
        }

        let Some(output_info) = &context.output else {
            return Ok(());
        };

        let filtered_message = Message {
            source: self.name.clone(),
            topic: output_info.name.clone(),
            payload: serde_json::json!({ output: value }),
            timestamp: message.timestamp,
        };

        let _ = output_info.channel.publish(filtered_message).await;

        Ok(())
    }
}

#[async_trait]
impl Processor for LowPassProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Lowpass processor [{}] initialised", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        context: &mut ProcessingContext,
    ) -> anyhow::Result<()> {
        if let Some((_, input)) = context.inputs.iter_mut().next() {
            select! {
                // Wait for a message from the input channel
                Some(message) = input.recv() => {
                    self.process_message(message, context).await?;
                }            
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => { }
            }
        }

        Ok(())
    }
}