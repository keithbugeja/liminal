use super::super::processor::Processor;

use crate::config::StageConfig;
use crate::core::context::ProcessingContext;

use async_trait::async_trait;

pub struct FusionStage {
    name: String,
}

impl FusionStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        Box::new(Self {
            name: name.to_string(),
        })
    }
}

#[async_trait]
impl Processor for FusionStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Fusion stage [{}] initialized", self.name);
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
