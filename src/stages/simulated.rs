use crate::config::StageConfig;
use crate::core::message::Message;
use crate::stages::{Input, Stage};
use async_trait::async_trait;

pub struct SimulatedInputStage {
    name: String,
    interval_ms: u64,
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
        })
    }
}

#[async_trait]
impl Stage for SimulatedInputStage {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn attach_input(&mut self, input: Input<Message>) {}

    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        tracing::info!("Simulated input stage is running");

        Ok(())
    }
}

// use super::stage::Stage;
// use crate::core::message::Message;
// use std::time::{SystemTime, UNIX_EPOCH};
// use tracing::{error, info};

// pub struct SimulatedInputSource {
//     name: String,
//     interval_ms: u64,
// }

// impl SimulatedInputSource {
//     pub fn new(name: &str, config: &StageConfig) -> Result<Self, String> {
//         Self::new(name, 100)
//     }
// }

// impl Stage for SimulatedInputSource {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn attach_input(&mut self, _input: Input<Message>) {
//         // No input to attach for simulated input
//     }

//     async fn init(&mut self) -> anyhow::Result<()> {
//         Ok(())
//     }

//     async fn run(self: Box<Self>) -> anyhow::Result<()> {
//         tracing::info!("Simulated input source [{}] is running", self.name);
//     }
// }
