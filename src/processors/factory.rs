use super::processor::Processor;
use super::input::SimulatedSignalProcessor;
use super::transform::{ScaleProcessor, LowPassFilterStage};
use super::aggregator::FusionStage;
use super::output::ConsoleLogProcessor;
use crate::config::StageConfig;

use std::collections::HashMap;
use std::sync::Mutex;

/// A type alias for a function that creates a processor.
type ProcessorConstructor = Box<dyn Fn(&str, StageConfig) -> Box<dyn Processor> + Send + Sync>;

lazy_static::lazy_static! {
    static ref PROCESSOR_REGISTRY: Mutex<HashMap<String, ProcessorConstructor>> = Mutex::new(HashMap::new());
}

/// Registers a processor constructor with the given name.
/// # Arguments
/// * `name` - The name of the processor.
/// * `constructor` - A function that creates the processor.
pub fn register_processor(name: &str, constructor: ProcessorConstructor) {
    let mut registry = PROCESSOR_REGISTRY.lock().unwrap();
    registry.insert(name.to_string(), constructor);
}

//// Creates a processor with the given name and configuration.
/// # Arguments
/// * `name` - The name of the processor.
/// * `config` - The configuration for the processor.
/// # Returns
/// * An `Option` containing the created processor, or `None` if the processor was not found.
pub fn create_processor(name: &str, config: StageConfig) -> Option<Box<dyn Processor>> {
    tracing::info!("Creating processor '{}'", name);

    let registry = PROCESSOR_REGISTRY.lock().unwrap();
    registry
        .get(name)
        .map(|constructor| constructor(name, config))
}

/// Registers default processors.
/// # Returns
/// * A result indicating success or failure.
pub fn create_processor_factories() -> anyhow::Result<()> {
    register_processor("simulated", Box::new(SimulatedSignalProcessor::new));
    register_processor("low_pass_filter", Box::new(LowPassFilterStage::new));
    register_processor("scale", Box::new(ScaleProcessor::new));
    register_processor("fusion", Box::new(FusionStage::new));
    register_processor("log", Box::new(ConsoleLogProcessor::new));

    tracing::info!("Default processors registered!");

    Ok(())
}
