use crate::config::StageConfig;
use crate::core::channel::PubSubChannel;
use crate::core::channel::Subscriber;
use crate::core::message::Message;
use crate::processors::processor::Processor;
use std::sync::Arc;

pub fn create_stage(name: &str, config: StageConfig) -> Option<Box<Stage>> {
    if let Some(processor) = crate::processors::create_processor(name, config) {
        Some(Box::new(Stage::new(name.to_string(), processor, None)))
    } else {
        tracing::error!("Stage processor '{}' not found", name);
        None
    }
}

pub enum ControlMessage {
    Terminate,
}

pub struct Stage {
    name: String,
    processor: Box<dyn Processor>,
    inputs: Vec<Subscriber<Message>>,
    output: Option<Arc<dyn PubSubChannel<Message>>>,
    conrol_channel: Option<tokio::sync::mpsc::Receiver<ControlMessage>>,
}

impl Stage {
    pub fn new(
        name: String,
        processor: Box<dyn Processor>,
        control_channel: Option<tokio::sync::mpsc::Receiver<ControlMessage>>,
    ) -> Self {
        Self {
            name,
            processor,
            inputs: Vec::new(),
            output: None,
            conrol_channel: control_channel,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn add_input(&mut self, input: Subscriber<Message>) {
        self.inputs.push(input);
        tracing::info!("Stage [{}] input added", self.name);
    }

    pub async fn add_output(&mut self, output: Arc<dyn PubSubChannel<Message>>) {
        self.output = Some(output);
        tracing::info!("Stage [{}] output set", self.name);
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        self.processor.init().await
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Stage [{}] is running", self.name);

        Ok(())
    }
}
