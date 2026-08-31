use crate::config::StageConfig;
use crate::core::context::ProcessingContext;
use crate::core::input_poll::{idle_sleep, try_one_each};
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

        let messages = try_one_each(&mut context.inputs).await;

        for input_message in &messages {
            tracing::info!(
                "'{}' => Message(source: {}, topic: {}, event_time: {:?}, ingestion_time: {:?}, sequence_id: {:?}, payload: {:?})",
                input_message.input_name,
                input_message.message.source,
                input_message.message.topic,
                input_message.message.timing.event_time,
                input_message.message.timing.ingestion_time,
                input_message.message.timing.sequence_id,
                input_message.message.payload
            );
        }

        // Small delay to prevent busy-waiting
        if messages.is_empty() {
            idle_sleep().await;
        }

        Ok(())
    }
}
