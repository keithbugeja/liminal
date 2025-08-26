use crate::processors::Processor;
use crate::config::{
    // extract_field_params, 
    extract_param, 
    ProcessorConfig, 
    StageConfig, 
    // FieldConfig
};
use crate::core::context::ProcessingContext;

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncWriteExt;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone)]
pub struct TcpOutputConfig {
    pub mode: TcpMode,       
    pub reconnect: bool,
    pub reconnect_interval_ms: u64,
}

#[derive(Debug, Clone)]
pub enum TcpMode {
    Client {host: String, port: u16},
    Server {bind_address: String},
}

impl ProcessorConfig for TcpOutputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let mode_str: String = extract_param(&config.parameters, "mode", "client".to_string());
        
        let mode = match mode_str.as_str() {
            "client" => {
                let host: String = extract_param(&config.parameters, "host", "localhost".to_string());
                let port: u16 = extract_param(&config.parameters, "port", 8080);
                TcpMode::Client { host, port }
            },
            "server" => {
                let bind_address: String = extract_param(&config.parameters, "bind_address", "0.0.0.0:8080".to_string());
                TcpMode::Server { bind_address }
            },
            _ => return Err(anyhow!("Invalid TCP mode: {}. Must be 'client' or 'server'", mode_str)),
        };

        let reconnect: bool = extract_param(&config.parameters, "reconnect", true);
        let reconnect_interval_ms: u64 = extract_param(&config.parameters, "reconnect_interval_ms", 5000);

        Ok(Self {
            mode,
            reconnect,
            reconnect_interval_ms,
        })
    }

    fn validate(&self) -> anyhow::Result<()> {
        match &self.mode {
            TcpMode::Client { host, port } => {
                if host.is_empty() {
                    return Err(anyhow!("TCP client host cannot be empty"));
                }
                if *port == 0 {
                    return Err(anyhow!("TCP client port must be greater than 0"));
                }
            },
            TcpMode::Server { bind_address } => {
                if bind_address.is_empty() {
                    return Err(anyhow!("TCP server bind_address cannot be empty"));
                }
            }
        }
        Ok(())
    }
}

pub struct TcpOutputProcessor {
    name: String,
    config: TcpOutputConfig,
    connection: Option<TcpStream>,
}

impl TcpOutputProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = TcpOutputConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            connection: None, 
        }))
    }

    async fn connect_client(&mut self) -> anyhow::Result<()> {
        if let TcpMode::Client {host, port} = &self.config.mode {
            tracing::info!("{}: Attempting to connect to TCP server at {}:{}", self.name, host, port);
            
            // Add timeout to prevent hanging
            match timeout(Duration::from_secs(10), TcpStream::connect(format!("{}:{}", host, port))).await {
                Ok(Ok(stream)) => {
                    self.connection = Some(stream);
                    tracing::info!("{}: Connected to TCP server at {}:{}", self.name, host, port);
                    Ok(())
                },
                Ok(Err(e)) => {
                    tracing::error!("{}: Failed to connect to TCP server at {}:{} - {}", self.name, host, port, e);
                    Err(anyhow!("Failed to connect to TCP server: {}", e))
                },
                Err(_) => {
                    tracing::error!("{}: Connection to TCP server at {}:{} timed out", self.name, host, port);
                    Err(anyhow!("Connection timeout"))
                }
            }
        } else {
            Err(anyhow!("connect_client called on server mode"))
        }
    }

    async fn wait_for_client(&mut self) -> anyhow::Result<()> {
        if let TcpMode::Server { bind_address } = &self.config.mode {
            let listener = TcpListener::bind(bind_address).await?;
            tracing::info!("{}: TCP server listening on {}", self.name, bind_address);
            
            // Accept one connection (P2P)
            let (stream, addr) = listener.accept().await?;
            tracing::info!("{}: Accepted TCP connection from {}", self.name, addr);
            
            self.connection = Some(stream);
            Ok(())
        } else {
            Err(anyhow!("wait_for_client called on client mode"))
        }
    }

    async fn send_message_with_length_prefix(&mut self, message: &[u8]) -> anyhow::Result<()> {
        if let Some(ref mut stream) = self.connection {
            // Send 4-byte length prefix (big-endian)
            let length = message.len() as u32;
            let length_bytes = length.to_be_bytes();
            
            stream.write_all(&length_bytes).await?;
            stream.write_all(message).await?;
            stream.flush().await?;
            
            Ok(())
        } else {
            Err(anyhow!("No TCP connection available"))
        }
    }

    async fn ensure_connection(&mut self) -> anyhow::Result<()> {
        if self.connection.is_none() {
            match &self.config.mode {
                TcpMode::Client { .. } => {
                    self.connect_client().await?;
                },
                TcpMode::Server { .. } => {
                    self.wait_for_client().await?;
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Processor for TcpOutputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("{}: TCP output processor initialised in {:?} mode", self.name, self.config.mode);
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Check if we have any messages to process first
        let mut has_messages = false;
        for (_, _subscriber) in &context.inputs {
            // Quick check without consuming messages  
            if !context.inputs.is_empty() {
                has_messages = true;
                break;
            }
        }

        // Only try to connect if we have messages to send or we're already connected
        if has_messages || self.connection.is_some() {
            if let Err(e) = self.ensure_connection().await {
                if self.config.reconnect {
                    tracing::debug!("{}: Connection failed, will retry in {}ms: {}", self.name, self.config.reconnect_interval_ms, e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(self.config.reconnect_interval_ms)).await;
                    return Ok(());
                } else {
                    return Err(e);
                }
            }
        }

        // Process messages from inputs
        for (_input_name, subscriber) in &mut context.inputs {
            while let Some(message) = subscriber.try_recv().await {
                tracing::debug!("{}: Processing message from {}", self.name, message.source);
                
                // Convert message to JSON and encode as UTF-8
                let json_value = serde_json::json!({
                    "source": message.source,
                    "topic": message.topic,
                    "payload": message.payload,
                    "timestamp": message.timestamp
                });
                let json_string = serde_json::to_string(&json_value)?;
                let json_bytes = json_string.into_bytes(); // UTF-8 encoding
                
                tracing::debug!("{}: Sending {} byte message", self.name, json_bytes.len());
                
                if let Err(e) = self.send_message_with_length_prefix(&json_bytes).await {
                    tracing::error!("{}: Failed to send message: {}", self.name, e);
                    
                    // Reset connection for reconnection attempt
                    self.connection = None;
                    
                    if !self.config.reconnect {
                        return Err(e);
                    }
                    break; // Exit message processing loop to attempt reconnection
                } else {
                    tracing::info!("{}: Successfully sent message", self.name);
                }
            }
        }

        Ok(())
    }
}