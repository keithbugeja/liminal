use super::processor::Processor;
use crate::config::StageConfig;
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;
use async_trait::async_trait;
use std::sync::Arc;

pub struct ScaleFilterStage {
    name: String,
    scale: f64,
}

impl ScaleFilterStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Processor> {
        let scale = config
            .parameters
            .as_ref()
            .and_then(|p| p.get("scale"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        Box::new(Self {
            name: name.to_string(),
            scale,
        })
    }
}

#[async_trait]
impl Processor for ScaleFilterStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("Scale filter stage [{}] initialized", self.name);
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
