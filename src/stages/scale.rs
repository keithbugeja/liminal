use crate::config::StageConfig;
use crate::core::message::Message;
use crate::stages::{Input, Stage};
use async_trait::async_trait;

pub struct ScaleFilterStage {
    name: String,
    scale: f64,
}

impl ScaleFilterStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Stage> {
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
impl Stage for ScaleFilterStage {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn attach_input(&mut self, input: Input<Message>) {}

    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        tracing::info!("Scale filter stage is running");

        Ok(())
    }
}
