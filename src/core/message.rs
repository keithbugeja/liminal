use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Message {
    pub source: String,
    pub topic: String,
    pub payload: Value,
    pub timestamp: u64,
}

impl Message {
    pub fn new(source: &str, topic: &str, payload: Value) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            source: source.to_string(),
            topic: topic.to_string(),
            payload,
            timestamp,
        }
    }
}
