use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub input_sources: HashMap<String, InputSource>,
    pub transforms: HashMap<String, Transform>,
    pub aggregations: HashMap<String, Aggregation>,
    pub output_sinks: HashMap<String, OutputSink>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InputSource {
    pub kind: String,
    pub topic: String,
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Transform {
    pub kind: String,
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Aggregation {
    pub kind: String,
    pub inputs: Vec<String>,
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OutputSink {
    pub kind: String,
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
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
