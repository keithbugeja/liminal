use crate::config::StageConfig;
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;
use crate::stages::Stage;
use async_trait::async_trait;
use std::sync::Arc;

pub struct ScaleFilterStage {
    name: String,
    scale: f64,
    input: Option<Subscriber<Message>>,
    output: Option<Arc<dyn PubSubChannel<Message>>>,
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
            input: None,
            output: None,
        })
    }
}

#[async_trait]
impl Stage for ScaleFilterStage {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn add_input(&mut self, input: Subscriber<Message>) {
        self.input = Some(input);

        tracing::info!("Scale stage [{}] input set", self.name);
    }

    fn add_output(&mut self, output: Arc<dyn PubSubChannel<Message>>) {
        self.output = Some(output);

        tracing::info!("Scale stage [{}] output set", self.name);
    }

    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Scale stage is running");

        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        tracing::info!("Scale stage is stopping");
        Ok(())
    }
}
