use super::processor::Processor;
use crate::config::StageConfig;
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;
use async_trait::async_trait;
use std::sync::Arc;

pub struct LogOutputStage {
    name: String,
}

impl LogOutputStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        Box::new(Self {
            name: name.to_string(),
        })
    }
}

#[async_trait]
impl Processor for LogOutputStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Log output stage [{}] initialized", self.name);
        Ok(())
    }

    async fn process(
        &mut self,
        inputs: Vec<Subscriber<Message>>,
        output: Option<Arc<dyn PubSubChannel<Message>>>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
