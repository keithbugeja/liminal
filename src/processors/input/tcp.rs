use crate::processors::Processor;
use crate::config::{extract_field_params, extract_param, StageConfig, FieldConfig};
use crate::config::ProcessorConfig;
use crate::core::message::Message;
use crate::core::context::ProcessingContext;
use crate::core::time::now_millis;

use async_trait::async_trait;
use tokio::net::{TcpListener, TcpStream};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

#[derive(Debug, Clone)]
pub struct TcpInputConfig {
    pub mode: String,       
    pub address: String,
    pub reconnect: bool,
    pub reconnect_interval_ms: u64,
}

pub enum TcpMode {
    Client {host: String, port: u16},
    Server {bind_address: String},
}

impl ProcessorConfig for TcpInputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        todo!()
    }

    fn validate(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct TcpInputProcessor {
    name: String,
    config: TcpInputConfig,
    connection: Option<TcpStream>,
}

impl TcpInputProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = TcpInputConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            connection: None, 
        }))
    }
}

#[async_trait]
impl Processor for TcpInputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        Ok(())
    }
}