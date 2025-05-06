use crate::config::InputSource;
use crate::message::Message;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;

pub mod mqtt;
pub mod simulated;

#[async_trait]
pub trait InputSourceHandler: Send + Sync {
    async fn run(
        &self,
        tx: Sender<Message>,
        mut shutdown: Receiver<()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub fn create_input_source_handler(
    name: &str,
    source: &InputSource,
) -> Result<Box<dyn InputSourceHandler>, String> {
    match source.kind.as_str() {
        "mqtt" => Ok(Box::new(mqtt::MqttInputSource::new(name, source)?)),
        "simulated" => Ok(Box::new(simulated::SimulatedInputSource::new(
            name, source,
        )?)),
        _ => Err(format!("Unsupported input source: {}", source.kind)),
    }
}
