use crate::config::{
    FieldConfig, ProcessorConfig, StageConfig, defaulted_param, extract_field_params,
};
use crate::core::context::ProcessingContext;
use crate::core::timing_mixin::{TimingMixin, WithTimingMixin};
use crate::processors::Processor;

use async_trait::async_trait;
use rand_distr::{Distribution, Normal, Uniform};
use std::time::SystemTime;
use tokio::time::Duration;

#[derive(Debug, Clone)]
pub struct SimulatedSignalConfig {
    pub interval_ms: u64,
    pub distribution: String,
    pub min_value: f64,
    pub max_value: f64,
    pub value_name: String,
    pub field: FieldConfig,
    pub timing: Option<crate::config::TimingConfig>,
}

impl ProcessorConfig for SimulatedSignalConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        // Extract simulation parameters with defaults
        let interval_ms = defaulted_param(&config.parameters, "interval_ms", 1000)?;
        let distribution =
            defaulted_param(&config.parameters, "distribution", "uniform".to_string())?;
        let min_value = defaulted_param(&config.parameters, "min_value", 0.0)?;
        let max_value = defaulted_param(&config.parameters, "max_value", 100.0)?;

        // Extract field configuration
        let field_config = extract_field_params(&config.parameters);
        let value_name = match &field_config {
            FieldConfig::OutputOnly(name) => name.clone(),
            _ => "value".to_string(),
        };

        // Extract timing configuration
        let timing_config = config.timing.clone();

        // Instantiate the configuration
        let config = Self {
            interval_ms,
            distribution,
            min_value,
            max_value,
            value_name,
            field: field_config,
            timing: timing_config,
        };

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.interval_ms == 0 {
            return Err(anyhow::anyhow!("interval_ms must be greater than 0"));
        }

        if self.min_value >= self.max_value {
            return Err(anyhow::anyhow!("min_value must be less than max_value"));
        }

        if !matches!(self.distribution.as_str(), "uniform" | "normal") {
            return Err(anyhow::anyhow!(
                "distribution must be 'uniform' or 'normal'"
            ));
        }

        Ok(())
    }
}

pub struct SimulatedSignalProcessor {
    name: String,
    config: SimulatedSignalConfig,
    timing: TimingMixin,
}

impl SimulatedSignalProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = SimulatedSignalConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        // Create timing mixin from processor configuration (respects ProcessorConfig pattern)
        let timing = TimingMixin::new(processor_config.timing.as_ref());

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            timing,
        }))
    }
}

#[async_trait]
impl Processor for SimulatedSignalProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!(
            "Simulated signal processor '{}' initialised with timing semantics",
            self.name
        );
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Generate a random value based on the specified distribution
        // and send it to the output channel. The rng is dropped before
        // the select statement to avoid blocking the async runtime.
        let value = {
            let mut rng = rand::rng();
            match self.config.distribution.as_str() {
                "uniform" => {
                    let uniform = Uniform::new(self.config.min_value, self.config.max_value)
                        .unwrap_or_else(|_| Uniform::new(0.0, 1.0).unwrap());
                    uniform.sample(&mut rng)
                }
                "normal" => {
                    let mean = (self.config.min_value + self.config.max_value) / 2.0;
                    let stddev = (self.config.max_value - self.config.min_value) / 6.0;
                    let normal = Normal::new(mean, stddev)
                        .unwrap_or_else(|_| Normal::new(0.0, 1.0).unwrap());
                    normal
                        .sample(&mut rng)
                        .clamp(self.config.min_value, self.config.max_value)
                }
                _ => {
                    tracing::warn!(
                        "Unknown distribution type: {}, using uniform",
                        self.config.distribution
                    );
                    let uniform = Uniform::new(self.config.min_value, self.config.max_value)
                        .unwrap_or_else(|_| Uniform::new(0.0, 1.0).unwrap());
                    uniform.sample(&mut rng)
                }
            }
        };

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(self.config.interval_ms)) => {
                // Single time capture to ensure consistency
                let event_time = SystemTime::now();

                let topic = if let Some(output_info) = &context.output {
                    output_info.name.clone()
                } else {
                    "simulated".to_string()
                };

                // Create payload (field : value)
                let payload = serde_json::json!({
                    self.config.value_name.clone(): value
                });

                // Create message using timing mixin
                let sequence_id = self.timing.next_sequence_id();
                let message = self.timing.create_message_with_event_time_extraction(
                    &self.name,
                    &topic,
                    payload,
                    event_time,
                ).with_sequence_id(sequence_id);

                tracing::debug!(
                    "Simulated signal generated: {} = {}, at {:?}, seq: {}, event_time: {:?}",
                    self.config.value_name,
                    value,
                    event_time,
                    sequence_id,
                    message.timing.event_time
                );

                if let Some(output_info) = &context.output {
                    let _ = output_info.channel.publish(message).await;
                }
            }
        }

        Ok(())
    }
}

impl WithTimingMixin for SimulatedSignalProcessor {
    fn timing_mixin(&self) -> &TimingMixin {
        &self.timing
    }

    fn timing_mixin_mut(&mut self) -> &mut TimingMixin {
        &mut self.timing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::collections::HashMap;

    fn stage_config(parameters: HashMap<String, Value>) -> StageConfig {
        StageConfig {
            r#type: "simulated".to_string(),
            inputs: None,
            output: Some("raw".to_string()),
            concurrency: None,
            channel: None,
            timing: None,
            parameters: Some(parameters),
        }
    }

    #[test]
    fn simulated_rejects_invalid_parameter_type() {
        let config = stage_config(HashMap::from([("interval_ms".to_string(), json!("fast"))]));

        let error = SimulatedSignalConfig::from_stage_config(&config)
            .expect_err("invalid interval type is rejected");

        assert!(error.to_string().contains("interval_ms"));
    }

    #[test]
    fn simulated_rejects_invalid_range() {
        let config = stage_config(HashMap::from([
            ("min_value".to_string(), json!(10.0)),
            ("max_value".to_string(), json!(1.0)),
        ]));

        let error = SimulatedSignalConfig::from_stage_config(&config)
            .expect_err("invalid range is rejected");

        assert!(error.to_string().contains("min_value"));
    }
}
