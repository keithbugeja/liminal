use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConcurrencyType {
    #[default]
    Thread,
    Pipeline,
    Owner,
}

/// Provides the default channel type, which is `Broadcast`.
fn default_concurrency_type() -> ConcurrencyType {
    ConcurrencyType::Thread
}

/// Configuration for stage concurrency.
#[derive(Clone, Debug, Deserialize)]
pub struct ConcurrencyConfig {
    #[serde(rename = "type", default = "default_concurrency_type")]
    pub r#type: ConcurrencyType,
}

/// Implements the `Default` trait for `ConcurrencyConfig`.
impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            r#type: default_concurrency_type(),
        }
    }
}

/// Enum representing the type of channel.
/// The `#[serde(rename_all = "lowercase")]` attribute ensures that the enum variants
/// are serialized and deserialized in lowercase.
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    /// Represents a broadcast channel (default). No backpressure.
    #[default]
    Broadcast,
    /// Represents a point-to-point multi-producer, single-consumer (MPSC) channel. Backpressure is applied.
    Direct,
    /// Represents a multi-producer, multi-consumer (MPMC) channel. Backpressure is applied.
    Shared,
    /// Represents a broadcast channel constructed using multiple tokio MPSCs channel. Backpressure is applied.
    Fanout,
}

/// Provides the default channel type, which is `Broadcast`.
fn default_channel_type() -> ChannelType {
    ChannelType::Broadcast
}

/// Provides the default capacity for a channel, which is `128`.
fn default_capacity() -> usize {
    128
}

/// Configuration for a channel.
/// Includes the type of channel and its capacity.
#[derive(Clone, Debug, Deserialize)]
pub struct ChannelConfig {
    #[serde(rename = "type", default = "default_channel_type")]
    pub r#type: ChannelType,
    #[serde(default = "default_capacity")]
    pub capacity: usize,
}

/// Implements the `Default` trait for `ChannelConfig`.
/// Provides default values for the channel configuration.
impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            r#type: default_channel_type(),
            capacity: default_capacity(),
        }
    }
}

/// Main configuration structure.
/// Contains configurations for inputs, pipelines, and outputs.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub inputs: HashMap<String, StageConfig>,
    pub pipelines: HashMap<String, PipelineConfig>,
    pub outputs: HashMap<String, StageConfig>,
}

/// Configuration for a stage.
/// Includes its type, inputs, output, channel configuration, and additional parameters.
#[derive(Clone, Debug, Deserialize)]
pub struct StageConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub inputs: Option<Vec<String>>,
    pub output: Option<String>,
    pub concurrency: Option<ConcurrencyConfig>,
    pub channel: Option<ChannelConfig>,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

/// Configuration for a pipeline.
/// Includes a description and a map of stage configurations.
#[derive(Clone, Debug, Deserialize)]
pub struct PipelineConfig {
    pub description: String,
    pub stages: HashMap<String, StageConfig>,
}

