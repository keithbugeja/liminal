use super::super::processor::Processor;

use crate::config::StageConfig;
use crate::core::context::ProcessingContext;

use async_trait::async_trait;

pub struct ConsoleLogProcessor {
    name: String,
}

impl ConsoleLogProcessor {
    pub fn new(name: &str, _ : StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        Ok(Box::new(Self {
            name: name.to_string(),
        }))
    }
}

#[async_trait]
impl Processor for ConsoleLogProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Log output stage [{}] initialized", self.name);
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Do nothing if there are no inputs
        if context.inputs.is_empty() {
            return Ok(());
        }
        
        for (name, input) in context.inputs.iter_mut() {
            if let Some(message) = input.try_recv().await {
                tracing::info!(
                    "Log output stage [{}] received message from [{}]: {:?}",
                    self.name, name, message
                );
            }
        }
        
        // Small delay to prevent busy-waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(())
    }
}
