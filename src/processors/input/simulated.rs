use crate::processors::Processor;

use crate::config::{extract_field_params, extract_param, StageConfig, FieldConfig};
use crate::core::message::Message;
use crate::core::context::ProcessingContext;
use crate::config::ProcessorConfig;
use crate::core::time::now_millis;

use async_trait::async_trait;
use rand_distr::{Distribution, Normal, Uniform};
use tokio::time::Duration;

#[derive(Debug, Clone)]
pub struct SimulatedSignalConfig {
    pub interval_ms: u64,
    pub distribution: String,
    pub min_value: f64,
    pub max_value: f64,
    pub value_name: String,
}

impl ProcessorConfig for SimulatedSignalConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let interval_ms = extract_param(&config.parameters, "interval_ms", 1000);
        let distribution = extract_param(&config.parameters, "distribution", "uniform".to_string());
        let min_value = extract_param(&config.parameters, "min_value", 0.0);
        let max_value = extract_param(&config.parameters, "max_value", 100.0);        

        let field_config = extract_field_params(&config.parameters);
        let value_name = match &field_config {
            FieldConfig::OutputOnly(name) => name.clone(),
            _ => "value".to_string(),
        };
        
        // Instantiate the configuration
        let config = Self {
            interval_ms,
            distribution,
            min_value,
            max_value,
            value_name,
        };
        
        // Validate the configuration
        if config.validate().is_err() {
            tracing::warn!("Invalid configuration for simulated signal stage: {:?}", config);
            return Err(anyhow::anyhow!("Invalid configuration for simulated signal stage"));
        }

        Ok(config)
    }   
} 
 
pub struct SimulatedSignalProcessor {
    name: String,
    config: SimulatedSignalConfig,
}

impl SimulatedSignalProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = SimulatedSignalConfig::from_stage_config(&config)?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
        }))
    }
}

#[async_trait]
impl Processor for SimulatedSignalProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Simulated signal processor '{}' initialised", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        context: &mut ProcessingContext,
    ) -> anyhow::Result<()> {
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
                    normal.sample(&mut rng).clamp(self.config.min_value, self.config.max_value)
                }
                _ => {
                    tracing::warn!("Unknown distribution type: {}, using uniform", self.config.distribution);
                    let uniform = Uniform::new(self.config.min_value, self.config.max_value)
                        .unwrap_or_else(|_| Uniform::new(0.0, 1.0).unwrap());
                    uniform.sample(&mut rng)
                }
            }
        }; 

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(self.config.interval_ms)) => {
                let now = now_millis();

                let topic = if let Some(output_info) = &context.output {
                    output_info.name.clone()
                } else {
                    "simulated".to_string()
                };

                let message = Message {
                    source: self.name.clone(),
                    topic,
                    payload: serde_json::json!({ &self.config.value_name : value }), // Fix: use value, not 1
                    timestamp: now,
                };

                if let Some(output_info) = &context.output {
                    let _ = output_info.channel.publish(message).await;
                }
            }
        }

        Ok(())
    }
}