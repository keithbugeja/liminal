use super::super::processor::Processor;

use crate::core::context::ProcessingContext;
use crate::config::{extract_param, StageConfig};

use async_trait::async_trait;

pub struct LowPassFilterStage {
    name: String,
    threshold: f64,
}

impl LowPassFilterStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let threshold = extract_param(&config.parameters, "threshold", 0.5);

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

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        for (name, input) in context.inputs.iter_mut() {
            if let Some(message) = input.recv().await {
                println!("Received from [{}]: {:?}", name, message);

                if let Some(output_info) = &context.output {
                    let _ = output_info.channel.publish(message).await;
                }
            }
        }
        Ok(())
    }
}
