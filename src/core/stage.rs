use super::channel::PubSubChannel;
use super::channel::Subscriber;
use super::message::Message;
use super::context::ProcessingContext;

use crate::config::StageConfig;
use crate::processors::processor::Processor;

use std::sync::Arc;

/// Creates a new stage with the given name and configuration.
///
/// # Arguments
/// * `name` - The name of the stage.
/// * `config` - The configuration for the stage.
///
/// # Returns
/// An `Option` containing a `Box<Stage>` if the stage was created successfully, or `None` if the processor was not found.
///
pub fn create_stage(name: &str, config: StageConfig) -> Option<Box<Stage>> {
    if let Ok(processor) = crate::processors::create_processor(name, config) {
        Some(Box::new(Stage::new(name.to_string(), processor, None)))
    } else {
        tracing::error!("Stage processor '{}' not found", name);
        None
    }
}

#[derive(Debug, Clone)]
pub enum ControlMessage {
    Terminate,
}

pub struct Stage {
    name: String,
    processor: Box<dyn Processor>,
    context: ProcessingContext,
    control_channel: Option<tokio::sync::broadcast::Receiver<ControlMessage>>,
}

impl Stage {
    pub fn new(
        name: String,
        processor: Box<dyn Processor>,
        control_channel: Option<tokio::sync::broadcast::Receiver<ControlMessage>>,
    ) -> Self {
        Self {
            name: name.clone(),
            processor,
            context: ProcessingContext::new(name),
            control_channel: control_channel,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attach_control_channel(
        &mut self,
        control_channel: tokio::sync::broadcast::Receiver<ControlMessage>,
    ) {
        self.control_channel = Some(control_channel);
        tracing::info!("Stage [{}] control channel attached", self.name);
    }

    pub async fn add_input(&mut self, name: &str, input: Subscriber<Message>) {
        self.context.add_input(name.to_string(), input);
        tracing::info!("Stage [{}] input added", self.name);
    }

    pub async fn add_output(&mut self, name: &str, output: Arc<dyn PubSubChannel<Message>>) {
        self.context.attach_output(name.to_string(), output);
        tracing::info!("Stage [{}] output set", self.name);
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        self.processor.init().await
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Stage [{}] is running", self.name);

        loop {
            tokio::select! {
                // Handle control messages
                Some(message) = async {
                    if let Some(control_channel) = &mut self.control_channel {
                        control_channel.recv().await.ok()
                    } else {
                        None
                    }
                } => {
                    match message {
                        ControlMessage::Terminate => {
                            tracing::info!("Stage [{}] received terminate signal", self.name);
                            break;
                        }
                    }
                }

                // Process messages
                result = self.processor.process(&mut self.context) => {
                    // Handle the result of the processor
                    if let Err(e) = result {
                        tracing::error!("Error in processor for stage [{}]: {}", self.name, e);
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }
}
