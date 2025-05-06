use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub input_sources: HashMap<String, InputSource>,
    pub transformations: HashMap<String, Transformation>,
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
pub struct Transformation {
    pub kind: String,
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Aggregation {
    pub kind: String,
    pub inputs: Vec<String>,
    pub parameters: Option<std::collections::HashMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OutputSink {
    pub kind: String,
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}
