use super::lowpass::LowPassFilterStage;
use super::processor::Processor;
use super::scale::ScaleFilterStage;
use super::simulated::SimulatedInputStage;
use crate::config::StageConfig;

use std::collections::HashMap;
use std::sync::Mutex;

type ProcessorConstructor = Box<dyn Fn(&str, StageConfig) -> Box<dyn Processor> + Send + Sync>;

lazy_static::lazy_static! {
    static ref PROCESSOR_REGISTRY: Mutex<HashMap<String, ProcessorConstructor>> = Mutex::new(HashMap::new());
}

pub fn register_processor(name: &str, constructor: ProcessorConstructor) {
    let mut registry = PROCESSOR_REGISTRY.lock().unwrap();
    registry.insert(name.to_string(), constructor);
}

pub fn create_processor(name: &str, config: StageConfig) -> Option<Box<dyn Processor>> {
    tracing::info!("Creating processor '{}'", name);

    let registry = PROCESSOR_REGISTRY.lock().unwrap();
    registry
        .get(name)
        .map(|constructor| constructor(name, config))
}

pub fn create_processor_factories() -> anyhow::Result<()> {
    register_processor("simulated", Box::new(SimulatedInputStage::new));
    register_processor("low_pass_filter", Box::new(LowPassFilterStage::new));
    register_processor("scale", Box::new(ScaleFilterStage::new));
    register_processor("fusion", Box::new(super::fusion::FusionStage::new));
    register_processor("log", Box::new(super::log::LogOutputStage::new));

    tracing::info!("Default processors registered!");

    Ok(())
}
