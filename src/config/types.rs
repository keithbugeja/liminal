//! Configuration Type Definitions
//! 
//! Core configuration structures for liminal. These types are deserialised 
//! from TOML configuration files and used to construct processing pipelines.

use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

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

/// Timing configuration for stages
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TimingConfig {
    /// Field in payload to use for event time (optional)
    pub event_time_field: Option<String>,
    
    /// Watermark generation strategy
    pub watermark_strategy: Option<WatermarkStrategy>,
    
    /// Maximum allowed lateness for out-of-order events (in milliseconds)
    #[serde(default = "default_max_lateness_ms")]
    pub max_lateness_ms: u64,
    
    /// Processing timeout for messages (in milliseconds)
    pub processing_timeout_ms: Option<u64>,
    
    /// Jitter bounds for real-time processing (in milliseconds)
    pub jitter_bounds_ms: Option<u64>,
    
    /// Enable timing metrics collection
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            event_time_field: None,
            watermark_strategy: None,
            max_lateness_ms: default_max_lateness_ms(),
            processing_timeout_ms: None,
            jitter_bounds_ms: None,
            metrics_enabled: default_metrics_enabled(),
        }
    }
}

impl TimingConfig {
    /// Convert to internal timing config with Duration types
    pub fn to_internal_config(&self) -> crate::core::timing::TimingConfig {
        crate::core::timing::TimingConfig {
            watermark_strategy: self.watermark_strategy.as_ref()
                .map(|ws| ws.to_internal())
                .unwrap_or(crate::core::timing::WatermarkStrategy::None),
            max_lateness: Duration::from_millis(self.max_lateness_ms),
            jitter_bounds: self.jitter_bounds_ms.map(Duration::from_millis),
            clock_source: crate::core::timing::ClockSource::System,
            metrics_enabled: self.metrics_enabled,
        }
    }
}

/// Watermark generation strategy configuration
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WatermarkStrategy {
    /// Generate watermarks periodically
    Periodic {
        /// Interval in milliseconds
        interval_ms: u64,
    },
    
    /// Generate watermarks based on field in payload
    Punctuated {
        /// Field name containing watermark timestamp
        field: String,
    },
    
    /// Heuristic watermarks based on event distribution
    Heuristic {
        /// Percentile to use (0-100)
        percentile: f64,
    },
}

impl WatermarkStrategy {
    fn to_internal(&self) -> crate::core::timing::WatermarkStrategy {
        match self {
            WatermarkStrategy::Periodic { interval_ms } => {
                crate::core::timing::WatermarkStrategy::Periodic {
                    interval: Duration::from_millis(*interval_ms),
                }
            }
            WatermarkStrategy::Punctuated { field } => {
                crate::core::timing::WatermarkStrategy::Punctuated {
                    field: field.clone(),
                }
            }
            WatermarkStrategy::Heuristic { percentile } => {
                crate::core::timing::WatermarkStrategy::Heuristic {
                    percentile: *percentile,
                }
            }
        }
    }
}

const fn default_max_lateness_ms() -> u64 {
    30_000 // 30 seconds
}

const fn default_metrics_enabled() -> bool {
    true
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
/// [inputs.sensor_data.timing]
/// event_time_field = "timestamp"
/// watermark_strategy = { type = "periodic", interval_ms = 1000 }
/// max_lateness_ms = 5000
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
    
    /// Timing configuration for this stage
    pub timing: Option<TimingConfig>,
    
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