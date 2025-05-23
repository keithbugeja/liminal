use super::processor::Processor;
use crate::config::StageConfig;
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;
use async_trait::async_trait;
use std::sync::Arc;

pub struct SimulatedInputStage {
    name: String,
    interval_ms: u64,
}

impl SimulatedInputStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let interval_ms = config
            .parameters
            .as_ref()
            .and_then(|p| p.get("interval_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Box::new(Self {
            name: name.to_string(),
            interval_ms,
        })
    }
}

#[async_trait]
impl Processor for SimulatedInputStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Simulated input stage [{}] initialized", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        inputs: Vec<Subscriber<Message>>,
        output: Option<Arc<dyn PubSubChannel<Message>>>,
    ) -> anyhow::Result<()> {
        // Simulate input processing
        Ok(())
    }
}
