use crate::config::StageConfig;
use crate::core::context::ProcessingContext;
use crate::processors::Processor;

use async_trait::async_trait;

pub struct ConsoleOutputProcessor {
    name: String,
}

impl ConsoleOutputProcessor {
    pub fn new(name: &str, _config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        Ok(Box::new(Self {
            name: name.to_string(),
        }))
    }
}

#[async_trait]
impl Processor for ConsoleOutputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Console output processor '{}' initialised", self.name);
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
                    "'{}' => Message(source: {}, topic: {}, event_time: {:?}, ingestion_time: {:?}, sequence_id: {:?}, payload: {:?})",
                    name,
                    message.source,
                    message.topic,
                    message.timing.event_time,
                    message.timing.ingestion_time,
                    message.timing.sequence_id,
                    message.payload
                );
            }
        }

        // Small delay to prevent busy-waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(())
    }
}
