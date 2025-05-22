use crate::config::StageConfig;
use crate::core::{
    channel::{PubSubChannel, Subscriber},
    message::Message,
};
use crate::stages::Stage;
use async_trait::async_trait;
use std::sync::Arc;

pub struct LogOutputStage {
    name: String,
    input: Option<Subscriber<Message>>,
}

impl LogOutputStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Stage> {
        Box::new(Self {
            name: name.to_string(),
            input: None,
        })
    }
}

#[async_trait]
impl Stage for LogOutputStage {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn add_input(&mut self, input: Subscriber<Message>) {
        self.input = Some(input);

        tracing::info!("Log output stage [{}] input set", self.name);
    }

    fn add_output(&mut self, _output: Arc<dyn PubSubChannel<Message>>) {}

    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Log output stage is running");

        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        tracing::info!("Log output stage is stopping");
        Ok(())
    }
}
