use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Message {
    pub source: String,
    pub topic: String,
    pub payload: Value,
    pub timestamp: u64,
}
