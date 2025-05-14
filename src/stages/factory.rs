use super::lowpass::LowPassFilterStage;
use super::scale::ScaleFilterStage;
use super::simulated::SimulatedInputStage;
use crate::{config::StageConfig, stages::Stage};
use std::collections::HashMap;
use std::sync::Mutex;

type StageConstructor = Box<dyn Fn(&str, StageConfig) -> Box<dyn Stage> + Send + Sync>;

lazy_static::lazy_static! {
    static ref STAGE_REGISTRY: Mutex<HashMap<String, StageConstructor>> = Mutex::new(HashMap::new());
}

pub fn register_stage(name: &str, constructor: StageConstructor) {
    let mut registry = STAGE_REGISTRY.lock().unwrap();
    registry.insert(name.to_string(), constructor);
}

pub fn create_stage(name: &str, config: StageConfig) -> Option<Box<dyn Stage>> {
    tracing::info!("Creating stage '{}'", name);

    let registry = STAGE_REGISTRY.lock().unwrap();
    registry
        .get(name)
        .map(|constructor| constructor(name, config))
}

pub fn create_stage_factories() -> anyhow::Result<()> {
    register_stage("simulated", Box::new(SimulatedInputStage::new));
    register_stage("low_pass_filter", Box::new(LowPassFilterStage::new));
    register_stage("scale", Box::new(ScaleFilterStage::new));
    register_stage("fusion", Box::new(super::fusion::FusionStage::new));
    register_stage("log", Box::new(super::log::LogOutputStage::new));

    tracing::info!("Default stages registered!");

    Ok(())
}
