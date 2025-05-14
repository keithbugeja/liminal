use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub inputs: HashMap<String, StageConfig>,
    pub pipelines: HashMap<String, PipelineConfig>,
    pub outputs: HashMap<String, StageConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StageConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub inputs: Option<Vec<String>>,
    pub output: Option<String>,
    pub channel: Option<ChannelConfig>,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChannelConfig {
    #[serde(rename = "type", default = "default_channel_type")]
    pub r#type: ChannelType,
    #[serde(default = "default_capacity")]
    pub capacity: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Broadcast,
    Mpsc,
    Flume,
    Fanout,
}

fn default_channel_type() -> ChannelType {
    ChannelType::Broadcast
}

fn default_capacity() -> usize {
    128
}

#[derive(Clone, Debug, Deserialize)]
pub struct PipelineConfig {
    pub description: String,
    pub stages: HashMap<String, StageConfig>,
}

pub fn extract_param<T>(
    params: &Option<HashMap<String, serde_json::Value>>,
    key: &str,
    default: T,
) -> T
where
    T: serde::de::DeserializeOwned + Clone,
{
    params
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(default)
}
