use anyhow::anyhow;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};
use crate::config::{extract_param, StageConfig};

#[derive(Debug, Clone)]
pub struct TcpConfig {
    pub mode: TcpMode,
    pub reconnect: bool,
    pub reconnect_interval_ms: u64,
}

#[derive(Debug, Clone)]
pub enum TcpMode {
    Client { host: String, port: u16 },
    Server { host: String, port: u16 },
}

impl TcpConfig {
    pub fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let mode_str: String = extract_param(&config.parameters, "mode", "client".to_string());
        
        let mode = match mode_str.as_str() {
            "client" => {
                let host: String = extract_param(&config.parameters, "host", "localhost".to_string());
                let port: u16 = extract_param(&config.parameters, "port", 8080);
                TcpMode::Client { host, port }
            },
            "server" => {
                let host: String = extract_param(&config.parameters, "host", "0.0.0.0".to_string());
                let port: u16 = extract_param(&config.parameters, "port", 8080);
                TcpMode::Server { host, port }
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

    pub fn validate(&self) -> anyhow::Result<()> {
        match &self.mode {
            TcpMode::Client { host, port } | TcpMode::Server { host, port } => {
                if host.is_empty() {
                    return Err(anyhow!("TCP host cannot be empty"));
                }
                if *port == 0 {
                    return Err(anyhow!("TCP port must be greater than 0"));
                }
            }
        }
        Ok(())
    }
}

pub struct TcpConnection {
    name: String,
    config: TcpConfig,
    stream: Option<TcpStream>,
}

impl TcpConnection {
    pub fn new(name: String, config: TcpConfig) -> Self {
        Self {
            name,
            config,
            stream: None,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    async fn connect_client(&mut self) -> anyhow::Result<()> {
        if let TcpMode::Client { host, port } = &self.config.mode {
            tracing::info!("{}: Attempting to connect to TCP server at {}:{}", self.name, host, port);
            
            match timeout(Duration::from_secs(10), TcpStream::connect(format!("{}:{}", host, port))).await {
                Ok(Ok(stream)) => {
                    self.stream = Some(stream);
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
        if let TcpMode::Server { host, port } = &self.config.mode {
            let bind_address = format!("{}:{}", host, port);
            let listener = TcpListener::bind(&bind_address).await?;
            tracing::info!("{}: TCP server listening on {}", self.name, bind_address);
            
            // Accept one connection (P2P)
            let (stream, addr) = listener.accept().await?;
            tracing::info!("{}: Accepted TCP connection from {}", self.name, addr);
            
            self.stream = Some(stream);
            Ok(())
        } else {
            Err(anyhow!("wait_for_client called on client mode"))
        }
    }

    pub async fn ensure_connection(&mut self) -> anyhow::Result<()> {
        if self.stream.is_none() {
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

    pub fn disconnect(&mut self) {
        self.stream = None;
    }

    pub async fn send_message_with_length_prefix(&mut self, message: &[u8]) -> anyhow::Result<()> {
        if let Some(ref mut stream) = self.stream {
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

    pub async fn receive_message_with_length_prefix(&mut self) -> anyhow::Result<Vec<u8>> {
        if let Some(ref mut stream) = self.stream {
            // Read 4-byte length prefix (big-endian)
            let mut length_buf = [0u8; 4];
            stream.read_exact(&mut length_buf).await?;
            let message_length = u32::from_be_bytes(length_buf) as usize;
            
            tracing::debug!("{}: Expecting message of length: {}", self.name, message_length);
            
            // Read the actual message
            let mut message_buf = vec![0u8; message_length];
            stream.read_exact(&mut message_buf).await?;
            
            Ok(message_buf)
        } else {
            Err(anyhow!("No TCP connection available"))
        }
    }

    pub fn should_reconnect(&self) -> bool {
        self.config.reconnect
    }

    pub fn reconnect_interval(&self) -> u64 {
        self.config.reconnect_interval_ms
    }
}
