use crate::config::Aggregation;
use crate::message::Message;
use async_trait::async_trait;
use fusion::FusionAggregator;

pub mod fusion;

#[async_trait]
pub trait Aggregator: Send + Sync {
    async fn aggregate(&self, inputs: Vec<Message>) -> Option<Message>;
}

pub fn create_aggregator(name: &str, config: &Aggregation) -> Result<Box<dyn Aggregator>, String> {
    match config.kind.as_str() {
        "fusion" => Ok(Box::new(FusionAggregator::new(&config.parameters))),
        _ => Err(format!("Unknown aggregator kind '{}'", config.kind)),
    }
}
