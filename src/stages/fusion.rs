use crate::config::StageConfig;
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;
use crate::stages::Stage;
use async_trait::async_trait;
use std::sync::Arc;

pub struct FusionStage {
    name: String,
    inputs: Vec<Subscriber<Message>>,
    output: Option<Arc<dyn PubSubChannel<Message>>>,
}

impl FusionStage {
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
            inputs: Vec::new(),
            output: None,
        })
    }
}

#[async_trait]
impl Stage for FusionStage {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn add_input(&mut self, input: Subscriber<Message>) {
        self.inputs.push(input);

        tracing::info!("Fusion stage [{}] input added", self.name);
    }

    fn add_output(&mut self, output: Arc<dyn PubSubChannel<Message>>) {
        self.output = Some(output);

        tracing::info!("Fusion stage [{}] output set", self.name);
    }

    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Fusion stage is running");

        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        tracing::info!("Fusion stage is stopping");
        Ok(())
    }
}
