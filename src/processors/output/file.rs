//! File Output Processor
//! 
//! Writes messages to files with configurable formatting and rotation options.
//! Supports JSON, CSV, and plain text output formats with automatic file
//! creation and directory handling.
//! 
//! Author: Keith Bugeja

use crate::processors::Processor;
use crate::config::{StageConfig, ProcessorConfig};
use crate::config::params::extract_param;
use crate::core::context::ProcessingContext;
use crate::core::message::Message;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use std::collections::HashMap;

/// Output format for file writing.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// JSON format - one JSON object per line
    Json,
    /// CSV format with headers
    Csv,
    /// Plain text format
    Text,
    /// Pretty-printed JSON (multi-line, indented)
    Pretty,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Json
    }
}

/// Configuration for the file output processor.
#[derive(Debug)]
pub struct FileOutputConfig {
    /// Path to the output file
    pub file_path: PathBuf,
    /// Output format
    pub format: OutputFormat,
    /// Whether to append to existing file or overwrite
    pub append: bool,
    /// Whether to create parent directories if they don't exist
    pub create_dirs: bool,
    /// Buffer size for writing (0 = unbuffered)
    pub buffer_size: usize,
    /// Whether to flush after each message
    pub auto_flush: bool,
}

impl ProcessorConfig for FileOutputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        // Extract file path (required)
        let file_path = extract_param(&config.parameters, "file_path", None::<String>)
            .ok_or_else(|| anyhow::anyhow!("file_path parameter is required for file output processor"))?;
        
        let file_path = PathBuf::from(file_path);
        
        // Extract optional parameters with sensible defaults
        let format = extract_param(&config.parameters, "format", OutputFormat::Json);
        let append = extract_param(&config.parameters, "append", true);
        let create_dirs = extract_param(&config.parameters, "create_dirs", true);
        let buffer_size = extract_param(&config.parameters, "buffer_size", 8192_usize);
        let auto_flush = extract_param(&config.parameters, "auto_flush", false);
        
        let config = Self {
            file_path,
            format,
            append,
            create_dirs,
            buffer_size,
            auto_flush,
        };
        
        config.validate()?;
        Ok(config)
    }
    
    fn validate(&self) -> anyhow::Result<()> {
        // Validate file path
        if self.file_path.to_string_lossy().is_empty() {
            return Err(anyhow::anyhow!("file_path cannot be empty"));
        }
        
        // Validate parent directory if create_dirs is false
        if !self.create_dirs {
            if let Some(parent) = self.file_path.parent() {
                if !parent.exists() {
                    return Err(anyhow::anyhow!(
                        "Parent directory '{}' does not exist and create_dirs is false",
                        parent.display()
                    ));
                }
            }
        }
        
        Ok(())
    }
}

/// File output processor that writes messages to files.
/// 
/// Supports multiple output formats and provides robust file handling
/// with configurable buffering and directory creation.
/// 
/// # Configuration Parameters
/// 
/// - `file_path` (required): Path to the output file
/// - `format`: Output format ("json", "csv", "text", "pretty")
/// - `append`: Whether to append to existing file (default: true)
/// - `create_dirs`: Whether to create parent directories (default: true)
/// - `buffer_size`: Write buffer size in bytes (default: 8192)
/// - `auto_flush`: Whether to flush after each message (default: false)
/// 
/// # Example Configuration
/// 
/// ```toml
/// [outputs.file_logger]
/// type = "file"
/// inputs = ["processed_data"]
/// parameters = {
///     file_path = "logs/output.jsonl",
///     format = "json",
///     append = true,
///     create_dirs = true,
///     buffer_size = 16384,
///     auto_flush = false
/// }
/// ```
pub struct FileOutputProcessor {
    name: String,
    config: FileOutputConfig,
    writer: Option<BufWriter<File>>,
    csv_headers_written: bool,
}

impl FileOutputProcessor {
    /// Creates a new file output processor.
    /// 
    /// # Arguments
    /// 
    /// * `name` - The name of this processor instance
    /// * `config` - The stage configuration containing file output parameters
    /// 
    /// # Returns
    /// 
    /// A boxed processor instance ready for use in the pipeline.
    /// 
    /// # Errors
    /// 
    /// Returns an error if:
    /// - Required parameters are missing
    /// - File path is invalid
    /// - Parent directories don't exist and `create_dirs` is false
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let file_config = FileOutputConfig::from_stage_config(&config)?;
        
        Ok(Box::new(Self {
            name: name.to_string(),
            config: file_config,
            writer: None,
            csv_headers_written: false,
        }))
    }
    
    /// Opens the output file and creates the buffered writer.
    async fn open_file(&mut self) -> anyhow::Result<()> {
        // Create parent directories if needed
        if self.config.create_dirs {
            if let Some(parent) = self.config.file_path.parent() {
                tokio::fs::create_dir_all(parent).await
                    .map_err(|e| anyhow::anyhow!(
                        "Failed to create directory '{}': {}",
                        parent.display(),
                        e
                    ))?;
            }
        }
        
        // Open the file with appropriate options
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(self.config.append)
            .truncate(!self.config.append)
            .open(&self.config.file_path)
            .await
            .map_err(|e| anyhow::anyhow!(
                "Failed to open file '{}': {}",
                self.config.file_path.display(),
                e
            ))?;
        
        // Create buffered writer
        let writer = if self.config.buffer_size > 0 {
            BufWriter::with_capacity(self.config.buffer_size, file)
        } else {
            BufWriter::new(file)
        };
        
        self.writer = Some(writer);
        
        tracing::info!(
            "File output processor '{}' opened file '{}' (format: {:?}, append: {})",
            self.name,
            self.config.file_path.display(),
            self.config.format,
            self.config.append
        );
        
        Ok(())
    }
    
    /// Writes a JSON payload to the file in the configured format.
    async fn write_message(&mut self, channel_name: &str, payload: &serde_json::Value) -> anyhow::Result<()> {
        let writer = self.writer.as_mut()
            .ok_or_else(|| anyhow::anyhow!("File writer not initialized"))?;
        
        match self.config.format {
            OutputFormat::Json => {
                let json_line = serde_json::to_string(payload)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize payload to JSON: {}", e))?;
                writer.write_all(json_line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
            
            OutputFormat::Pretty => {
                let json_pretty = serde_json::to_string_pretty(payload)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize payload to pretty JSON: {}", e))?;
                writer.write_all(json_pretty.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
            
            OutputFormat::Csv => {
                let payload_obj = payload.as_object()
                    .ok_or_else(|| anyhow::anyhow!("CSV format requires JSON object payload"))?;
                                  
                // Write CSV data
                let values: Vec<String> = payload_obj.values()
                    .map(|v| match v {
                        serde_json::Value::String(s) => format!("\"{}\"", s.replace("\"", "\"\"")),
                        _ => v.to_string(),
                    })
                    .collect();
                let csv_line = values.join(",");
                writer.write_all(csv_line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
            
            OutputFormat::Text => {
                let text_line = format!("[{}] {}", channel_name, serde_json::to_string(payload)?);
                writer.write_all(text_line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
        
        // Auto-flush if configured
        if self.config.auto_flush {
            writer.flush().await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl Processor for FileOutputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        self.open_file().await?;
        tracing::info!("File output processor '{}' initialised", self.name);
        Ok(())
    }
    
    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Skip processing if no inputs
        if context.inputs.is_empty() {
            return Ok(());
        }
        
        let mut messages_written = 0;
        
        // Process messages from all input channels
        for (channel_name, input) in context.inputs.iter_mut() {
            while let Some(message) = input.try_recv().await {
                let payload = message.payload;
                
                self.write_message(channel_name, &payload).await
                    .map_err(|e| anyhow::anyhow!(
                        "Failed to write message from channel '{}': {}",
                        channel_name,
                        e
                    ))?;
                messages_written += 1;
            }
        }
        
        // Flush periodically even if auto_flush is disabled
        if messages_written > 0 && !self.config.auto_flush {
            if let Some(writer) = self.writer.as_mut() {
                writer.flush().await?;
            }
        }
        
        // Small delay to prevent busy-waiting when no messages
        if messages_written == 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        Ok(())
    }
}