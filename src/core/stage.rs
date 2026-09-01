use super::channel::PubSubChannel;
use super::channel::Subscriber;
use super::context::ProcessingContext;
use super::message::Message;
use super::runtime_event::{RuntimeEvent, RuntimeEventKind};
use super::runtime_observer::{SharedRuntimeObserver, noop_runtime_observer};

use crate::config::StageConfig;
use crate::processors::processor::Processor;

use anyhow::{Context, Result};
use std::sync::Arc;

/// Creates a new stage with the given name and configuration.
///
/// # Arguments
/// * `name` - The name of the stage.
/// * `config` - The configuration for the stage.
///
/// # Returns
/// A `Box<Stage>` if the stage and its processor were created successfully.
///
pub fn create_stage(name: &str, config: StageConfig) -> Result<Box<Stage>> {
    // Uncomment if the stage name is used as processor type
    // if let Ok(processor) = crate::processors::create_processor(name, config) {

    let processor_type = config.r#type.clone();
    let processor =
        crate::processors::create_processor(&processor_type, config).with_context(|| {
            format!(
                "failed to create processor '{}' for stage '{}'",
                processor_type, name
            )
        })?;

    Ok(Box::new(Stage::new(
        name.to_string(),
        processor_type,
        processor,
        None,
    )))
}

#[derive(Debug, Clone)]
pub enum ControlMessage {
    Terminate,
}

pub struct Stage {
    name: String,
    processor_type: String,
    processor: Box<dyn Processor>,
    context: ProcessingContext,
    control_channel: Option<tokio::sync::broadcast::Receiver<ControlMessage>>,
    observer: SharedRuntimeObserver,
}

impl Stage {
    pub fn new(
        name: String,
        processor_type: String,
        processor: Box<dyn Processor>,
        control_channel: Option<tokio::sync::broadcast::Receiver<ControlMessage>>,
    ) -> Self {
        let mut context = ProcessingContext::new(name.clone());
        context.set_runtime_metadata(processor_type.clone(), noop_runtime_observer());

        Self {
            name: name.clone(),
            processor_type,
            processor,
            context,
            control_channel: control_channel,
            observer: noop_runtime_observer(),
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
    }

    pub fn set_observer(&mut self, observer: SharedRuntimeObserver) {
        self.observer = observer.clone();
        self.context
            .set_runtime_metadata(self.processor_type.clone(), observer);
    }

    pub async fn add_input(&mut self, name: &str, input: Subscriber<Message>) {
        self.context.add_input(name.to_string(), input);
    }

    pub async fn add_output(&mut self, name: &str, output: Arc<dyn PubSubChannel<Message>>) {
        self.context.attach_output(name.to_string(), output);
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        self.processor.init().await
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Stage '{}' is running", self.name);
        self.observer.emit(
            RuntimeEvent::new(RuntimeEventKind::StageRunning)
                .stage(self.name.clone())
                .processor_type(self.processor_type.clone()),
        );

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
                            tracing::info!("Stage '{}' received terminate signal", self.name);
                            break;
                        }
                    }
                }

                // Process messages
                result = self.processor.process(&mut self.context) => {
                    // Handle the result of the processor
                    if let Err(e) = result {
                        tracing::error!("Error in processor for stage '{}': {}", self.name, e);
                        self.observer.emit(
                            RuntimeEvent::new(RuntimeEventKind::ProcessorError)
                                .stage(self.name.clone())
                                .processor_type(self.processor_type.clone())
                                .text(e.to_string()),
                        );
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }
}
