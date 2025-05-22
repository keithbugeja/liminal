use crate::config::StageConfig;
use crate::core::channel::{PubSubChannel, Subscriber};
use crate::core::message::Message;
use crate::stages::Stage;
use async_trait::async_trait;
use std::sync::{Arc, RwLock};

pub struct SimulatedInputStage {
    name: String,
    interval_ms: u64,
    output: Option<Arc<dyn PubSubChannel<Message>>>,
}

impl SimulatedInputStage {
    pub fn new(name: &str, config: StageConfig) -> Box<dyn Stage> {
        let interval_ms = config
            .parameters
            .as_ref()
            .and_then(|p| p.get("interval_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Box::new(Self {
            name: name.to_string(),
            interval_ms,
            output: None,
        })
    }
}

#[async_trait]
impl Stage for SimulatedInputStage {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn add_input(&mut self, _input: Subscriber<Message>) {
        todo!()
    }

    fn add_output(&mut self, output: Arc<dyn PubSubChannel<Message>>) {
        self.output = Some(output);

        tracing::info!("Simulated input stage [{}] output set", self.name);
    }

    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Simulated input stage is running");

        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        tracing::info!("Simulated input stage is stopping");
        Ok(())
    }
}
