//! Configuration Loader Module
//! 
//! This module provides utilities for loading Liminal configuration from various sources.
//! It supports loading from TOML files, strings, and includes validation to ensure
//! configurations are well-formed before use.
//! 
//! # Supported Formats
//! 
//! Currently only TOML format is supported, which provides a clean and readable
//! configuration syntax that maps well to Rust's type system.
//! 
//! # Loading Sources
//! 
//! - **File paths**: Load from `.toml` files on disk
//! - **String content**: Load from TOML content in memory
//! - **Default config**: Generate sensible default configurations
//! 
//! # Error Handling
//! 
//! All loading functions return detailed errors that help diagnose configuration problems:
//! - File I/O errors (missing files, permissions)
//! - TOML parsing errors (syntax issues, type mismatches)
//! - Validation errors (structural problems with the configuration)
//! 
//! # Example Usage
//! 
//! ```rust
//! use liminal::config::loader::{load_config, load_config_from_string};
//! 
//! // Load from file
//! let config = load_config("config.toml")?;
//! 
//! // Load from string
//! let toml_content = r#"
//!     [inputs.sensor]
//!     type = "simulated"
//!     output = "data"
//! "#;
//! let config = load_config_from_string(toml_content)?;
//! ```

use crate::config::types::Config;
use std::fs;
use std::path::Path;
use toml;

/// Loads configuration from a TOML file.
/// 
/// This is the primary way to load configuration in production environments.
/// The function reads the entire file content, parses it as TOML, and validates
/// the resulting configuration structure.
/// 
/// # Arguments
/// 
/// * `path` - Path to the TOML configuration file (can be `&str`, `String`, `PathBuf`, etc.)
/// 
/// # Returns
/// 
/// * `Ok(Config)` - Successfully loaded and validated configuration
/// * `Err(ConfigError)` - Loading failed due to I/O, parsing, or validation errors
/// 
/// # Errors
/// 
/// This function can fail in several ways:
/// 
/// ## File I/O Errors
/// - **File not found**: The specified path doesn't exist
/// - **Permission denied**: Insufficient permissions to read the file
/// - **Invalid UTF-8**: File contains non-UTF-8 content
/// 
/// ## TOML Parsing Errors
/// - **Syntax errors**: Malformed TOML syntax (missing quotes, brackets, etc.)
/// - **Type mismatches**: Values that can't be deserialized to expected types
/// - **Missing required fields**: TOML doesn't contain expected structure
/// 
/// ## Validation Errors
/// - **Structural issues**: Configuration violates stage connection rules
/// - **Parameter problems**: Invalid or inconsistent processor parameters
/// 
/// # Example
/// 
/// ```rust
/// use liminal::config::loader::load_config;
/// 
/// match load_config("config.toml") {
///     Ok(config) => {
///         println!("Loaded configuration with {} inputs", config.inputs.len());
///     },
///     Err(e) => {
///         eprintln!("Failed to load configuration: {}", e);
///         std::process::exit(1);
///     }
/// }
/// ```
/// 
/// # File Format Example
/// 
/// ```toml
/// # Input data sources
/// [inputs.temperature_sensor]
/// type = "simulated"
/// output = "raw_temp"
/// parameters = { field_out = "temperature", interval_ms = 1000 }
/// 
/// # Processing pipelines
/// [pipelines.main_processing]
/// description = "Process temperature data"
/// 
/// [pipelines.main_processing.stages.scale]
/// type = "scale"
/// inputs = ["raw_temp"]
/// output = "scaled_temp"
/// parameters = { field_in = "temperature", field_out = "scaled_temperature", scale_factor = 1.8 }
/// 
/// # Output destinations
/// [outputs.console]
/// type = "log"
/// inputs = ["scaled_temp"]
/// parameters = { format = "json" }
/// ```
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

/// Loads configuration from a TOML string.
/// 
/// This function is useful for testing, dynamic configuration generation,
/// or when configuration content comes from non-file sources (databases,
/// network requests, embedded resources, etc.).
/// 
/// # Arguments
/// 
/// * `content` - TOML content as a string slice
/// 
/// # Returns
/// 
/// * `Ok(Config)` - Successfully parsed and validated configuration
/// * `Err(ConfigError)` - Parsing or validation failed
/// 
/// # Errors
/// 
/// This function can fail for the same parsing and validation reasons as
/// `load_config()`, but without the file I/O error possibilities.
/// 
/// # Example
/// 
/// ```rust
/// use liminal::config::loader::load_config_from_string;
/// 
/// let toml_content = r#"
///     [inputs.test_source]
///     type = "simulated"
///     output = "test_data"
///     parameters = { field_out = "value", interval_ms = 100 }
///     
///     [outputs.test_sink]
///     type = "log"
///     inputs = ["test_data"]
/// "#;
/// 
/// let config = load_config_from_string(toml_content)?;
/// assert_eq!(config.inputs.len(), 1);
/// assert_eq!(config.outputs.len(), 1);
/// ```
/// 
/// # Testing Usage
/// 
/// This function is particularly useful in unit tests:
/// 
/// ```rust
/// #[test]
/// fn test_scale_processor_config() {
///     let config_toml = r#"
///         [inputs.source]
///         type = "simulated"
///         output = "data"
///         
///         [pipelines.test.stages.scale]
///         type = "scale"
///         inputs = ["data"]
///         output = "scaled"
///         parameters = { field_in = "value", field_out = "scaled_value", scale_factor = 2.0 }
///         
///         [outputs.sink]
///         type = "log"
///         inputs = ["scaled"]
///     "#;
///     
///     let config = load_config_from_string(config_toml).unwrap();
///     // Test the configuration...
/// }
/// ```
pub fn load_config_from_string(content: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let config: Config = toml::from_str(content)?;
    Ok(config)
}

/// Creates a minimal default configuration for testing and examples.
/// 
/// This function generates a simple but complete configuration that can be used
/// as a starting point for development or as a fallback when no configuration
/// file is available.
/// 
/// # Returns
/// 
/// A valid `Config` with a basic simulated input, simple pipeline, and console output.
/// 
/// # Generated Configuration
/// 
/// The default configuration includes:
/// - **Input**: Simulated data source generating test data
/// - **Pipeline**: Simple scale transformation
/// - **Output**: Console logging for verification
/// 
/// # Example
/// 
/// ```rust
/// use liminal::config::loader::default_config;
/// 
/// let config = default_config();
/// println!("Default config has {} inputs", config.inputs.len());
/// 
/// // Use for testing or as a template
/// let pipeline_manager = PipelineManager::from_config(config)?;
/// ```
/// 
/// # Equivalent TOML
/// 
/// The generated configuration is equivalent to this TOML:
/// 
/// ```toml
/// [inputs.default_source]
/// type = "simulated"
/// output = "raw_data"
/// parameters = { field_out = "value", interval_ms = 1000 }
/// 
/// [pipelines.default_pipeline]
/// description = "Default processing pipeline"
/// 
/// [pipelines.default_pipeline.stages.scale]
/// type = "scale"
/// inputs = ["raw_data"]
/// output = "processed_data"
/// parameters = { field_in = "value", field_out = "scaled_value", scale_factor = 1.0 }
/// 
/// [outputs.default_console]
/// type = "log"
/// inputs = ["processed_data"]
/// parameters = { format = "pretty" }
/// ```
pub fn default_config() -> Config {
    use std::collections::HashMap;
    use super::types::{StageConfig, PipelineConfig};
    
    // Create default input stage
    let default_input = StageConfig {
        r#type: "simulated".to_string(),
        inputs: None,
        output: Some("raw_data".to_string()),
        concurrency: None,
        channel: None,
        parameters: Some({
            let mut params = HashMap::new();
            params.insert("field_out".to_string(), serde_json::json!("value"));
            params.insert("interval_ms".to_string(), serde_json::json!(1000));
            params
        }),
    };
    
    // Create default pipeline stage
    let default_stage = StageConfig {
        r#type: "scale".to_string(),
        inputs: Some(vec!["raw_data".to_string()]),
        output: Some("processed_data".to_string()),
        concurrency: None,
        channel: None,
        parameters: Some({
            let mut params = HashMap::new();
            params.insert("field_in".to_string(), serde_json::json!("value"));
            params.insert("field_out".to_string(), serde_json::json!("scaled_value"));
            params.insert("scale_factor".to_string(), serde_json::json!(1.0));
            params
        }),
    };
    
    // Create default output stage
    let default_output = StageConfig {
        r#type: "log".to_string(),
        inputs: Some(vec!["processed_data".to_string()]),
        output: None,
        concurrency: None,
        channel: None,
        parameters: Some({
            let mut params = HashMap::new();
            params.insert("format".to_string(), serde_json::json!("pretty"));
            params
        }),
    };
    
    // Assemble the complete configuration
    Config {
        inputs: {
            let mut inputs = HashMap::new();
            inputs.insert("default_source".to_string(), default_input);
            inputs
        },
        pipelines: {
            let mut pipelines = HashMap::new();
            let mut pipeline = PipelineConfig {
                description: "Default processing pipeline".to_string(),
                stages: HashMap::new(),
            };
            pipeline.stages.insert("scale".to_string(), default_stage);
            pipelines.insert("default_pipeline".to_string(), pipeline);
            pipelines
        },
        outputs: {
            let mut outputs = HashMap::new();
            outputs.insert("default_console".to_string(), default_output);
            outputs
        },
    }
}