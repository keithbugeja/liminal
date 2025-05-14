use crate::config::StageConfig;
use crate::core::message::Message;
use crate::stages::{Input, Stage};
use async_trait::async_trait;

pub struct LogOutputStage {
    name: String,
}

impl LogOutputStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Stage> {
        // let threshold = config
        //     .parameters
        //     .as_ref()
        //     .and_then(|p| p.get("threshold"))
        //     .and_then(|v| v.as_f64())
        //     .unwrap_or(0.5);

        Box::new(Self {
            name: name.to_string(),
            // threshold,
        })
    }
}

#[async_trait]
impl Stage for LogOutputStage {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn attach_input(&mut self, input: Input<Message>) {}

    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        tracing::info!("Log output stage is running");

        Ok(())
    }
}
