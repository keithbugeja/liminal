//! Configuration Type Definitions
//! 
//! Core configuration structures for liminal. These types are deserialised 
//! from TOML configuration files and used to construct processing pipelines.

use serde::Deserialize;
use std::collections::HashMap;

/// Concurrency execution model for stages.
/// 
/// Currently all variants execute as single-threaded stages.
/// Different types are reserved for future concurrency implementations.

#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConcurrencyType {
    /// Single dedicated thread per stage (default and current implementation)
    #[default]
    Thread,
    
    /// Pipeline-style concurrent execution (future enhancement)
    Pipeline,
    
    /// User-managed threading (future enhancement)
    Owner,
}

/// Configuration for stage concurrency behaviour.
/// 
/// Currently all concurrency types execute as single-threaded stages.
/// The configuration is preserved for future compatibility when enhanced
/// concurrency models are implemented.
#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq)]
pub struct ConcurrencyConfig {
    /// The concurrency model to use for this stage
    #[serde(rename = "type", default)]
    pub r#type: ConcurrencyType,
}

/// Communication channel type between stages.
/// 
/// Different channel types offer different trade-offs between performance,
/// reliability, and backpressure handling.
#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    /// Broadcast channel with no backpressure (default)
    /// 
    /// Fast fan-out messaging where slow consumers may miss messages.
    /// Best for real-time data streams where latest data is most important.
    #[default]
    Broadcast,
    
    /// Point-to-point MPSC channel with backpressure
    /// 
    /// Single producer, single consumer with reliable delivery.
    /// Producer will wait if consumer falls behind.
    Direct,
    
    /// Multi-consumer MPMC channel with backpressure
    /// 
    /// Multiple consumers share the message load. Each message is
    /// delivered to exactly one consumer. Good for work distribution.
    Shared,
    
    /// Fan-out using multiple MPSC channels with backpressure
    /// 
    /// Each consumer gets a copy of every message with reliable delivery.
    /// Producer will wait if any consumer falls behind.
    Fanout,
}

/// Configuration for inter-stage communication channels.
/// 
/// Defines how messages flow between processing stages, including
/// the communication pattern and buffer capacity.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ChannelConfig {
    /// The type of channel to create
    #[serde(rename = "type", default)]
    pub r#type: ChannelType,
    
    /// Maximum number of messages the channel can buffer
    #[serde(default = "default_capacity")]
    pub capacity: usize,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            r#type: ChannelType::default(),
            capacity: default_capacity(),
        }
    }
}

/// Provides the default capacity for channels.
const fn default_capacity() -> usize {
    128
}

/// Root configuration for the entire liminal system.
/// 
/// Contains all configuration needed to set up data processing pipelines,
/// including input sources, processing stages, and output destinations.
/// 
/// # Example Structure
/// 
/// ```toml
/// [inputs.sensor_data]
/// type = "simulated"
/// output = "raw_data"
/// 
/// [pipelines.processing.stages.filter]
/// type = "lowpass"
/// inputs = ["raw_data"]
/// output = "filtered_data"
/// 
/// [outputs.console]
/// type = "log"
/// inputs = ["filtered_data"]
/// ```
#[derive(Clone, Debug, Deserialize, Default)]
pub struct Config {
    /// Input stage configurations - data sources that generate messages
    #[serde(default)]
    pub inputs: HashMap<String, StageConfig>,
    
    /// Pipeline configurations - multi-stage processing workflows
    #[serde(default)]
    pub pipelines: HashMap<String, PipelineConfig>,
    
    /// Output stage configurations - data sinks that consume messages
    #[serde(default)]
    pub outputs: HashMap<String, StageConfig>,
}

/// Configuration for an individual processing stage.
/// 
/// A stage represents a single step in the data processing pipeline.
/// Different stage types (input, transform, output) have different
/// requirements for inputs and outputs.
/// 
/// # Stage Types
/// 
/// - **Input stages**: Generate data, have `output` but no `inputs`
/// - **Transform stages**: Process data, have both `inputs` and `output`
/// - **Output stages**: Consume data, have `inputs` but no `output`
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StageConfig {
    /// The processor type to instantiate (e.g., "simulated", "scale", "log")
    #[serde(rename = "type")]
    pub r#type: String,
    
    /// Input data stream names this stage consumes from
    pub inputs: Option<Vec<String>>,
    
    /// Output data stream name this stage produces to
    pub output: Option<String>,
    
    /// Concurrency configuration (currently unused, reserved for future)
    pub concurrency: Option<ConcurrencyConfig>,
    
    /// Channel configuration for this stage's output communication
    pub channel: Option<ChannelConfig>,
    
    /// Processor-specific configuration parameters
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

/// Configuration for a multi-stage processing pipeline.
/// 
/// Pipelines contain multiple stages that process data in sequence or parallel,
/// depending on their input/output data stream connections.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PipelineConfig {
    /// Human-readable description of the pipeline's purpose
    pub description: String,
    
    /// Map of stage name to stage configuration
    pub stages: HashMap<String, StageConfig>,
}