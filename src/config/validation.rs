//! Configuration Validation Module
//! 
//! This module provides validation functions for Liminal configuration structures.
//! It ensures that configurations are structurally sound and follow the expected
//! patterns for different stage types before pipeline construction begins.
//! 
//! # Validation Rules
//! 
//! ## Input Stages
//! - Must have an output data stream
//! - Must not have input data streams
//! - Field configuration must be output-only or none
//! 
//! ## Pipeline Stages (Transform)
//! - Must have at least one input data stream
//! - Must have exactly one output data stream
//! - Field configuration is processor-specific
//! 
//! ## Output Stages
//! - Must have at least one input data stream
//! - Must not have an output data stream
//! - Field configuration is processor-specific
//! 
//! # Example Usage
//! 
//! ```rust
//! use liminal::config::validation::validate_config;
//! 
//! let config = load_config_from_file("config.toml")?;
//! validate_config(&config)?;
//! println!("Configuration is valid!");
//! ```

use crate::config::types::*;
use crate::config::params::extract_field_params;
use crate::config::field::FieldConfig;

/// Validates the entire Liminal configuration for structural correctness.
/// 
/// This function performs comprehensive validation of all configuration sections,
/// ensuring that each stage type follows its expected input/output patterns and
/// that field configurations are appropriate for each stage category.
/// 
/// # Arguments
/// 
/// * `config` - The root configuration structure to validate
/// 
/// # Returns
/// 
/// * `Ok(())` - Configuration is valid and ready for pipeline construction
/// * `Err(anyhow::Error)` - Configuration has validation errors
/// 
/// # Validation Process
/// 
/// 1. **Input Stage Validation** - Ensures input stages generate data correctly
/// 2. **Pipeline Stage Validation** - Ensures transform stages process data correctly  
/// 3. **Output Stage Validation** - Ensures output stages consume data correctly
/// 
/// # Errors
/// 
/// This function will return an error if:
/// - Input stages have input streams configured
/// - Input stages don't have output streams configured
/// - Pipeline stages don't have both inputs and outputs
/// - Output stages have output streams configured
/// - Output stages don't have input streams configured
/// - Field configurations are inappropriate for stage types
/// 
/// # Example
/// 
/// ```rust
/// let config = Config {
///     inputs: HashMap::from([("sensor".to_string(), StageConfig { /* ... */ })]),
///     pipelines: HashMap::new(),
///     outputs: HashMap::from([("console".to_string(), StageConfig { /* ... */ })]),
/// };
/// 
/// match validate_config(&config) {
///     Ok(()) => println!("Configuration is valid"),
///     Err(e) => eprintln!("Validation failed: {}", e),
/// }
/// ```
pub fn validate_config(config: &Config) -> anyhow::Result<()> {
    // Validate all input stages - these generate data into the system
    for (name, stage_config) in &config.inputs {
        validate_input_stage(name, stage_config)?;
    }

     // Validate all pipeline configurations - these transform data
    for (name, pipeline_config) in &config.pipelines {
        validate_pipeline(name, pipeline_config)?;
    }
    
    // Validate all output stages - these consume data from the system
    for (name, stage_config) in &config.outputs {
        validate_output_stage(name, stage_config)?;
    }

    Ok(())
}

/// Validates an input stage configuration.
/// 
/// Input stages are data sources that generate messages into the processing
/// pipeline. They should only produce output and never consume input.
/// 
/// # Arguments
/// 
/// * `name` - The name/identifier of the input stage
/// * `config` - The stage configuration to validate
/// 
/// # Returns
/// 
/// * `Ok(())` - Stage configuration is valid for an input stage
/// * `Err(anyhow::Error)` - Stage configuration violates input stage rules
/// 
/// # Validation Rules
/// 
/// - **No inputs allowed**: Input stages generate data, they don't consume it
/// - **Output required**: Input stages must specify where to send generated data
/// - **Field config**: Must be `OutputOnly` or `None` (input stages don't transform input fields)
/// 
/// # Example Valid Input Stage
/// 
/// ```toml
/// [inputs.temperature_sensor]
/// type = "simulated"
/// output = "raw_temperature"
/// parameters = { field_out = "temperature", interval_ms = 1000 }
/// ```
fn validate_input_stage(name: &str, config: &StageConfig) -> anyhow::Result<()> {
    // Input stages should not consume any data streams
    if config.inputs.is_some() {
        return Err(anyhow::anyhow!("Input stage '{}' should not have inputs", name));
    }

    // Input stages must specify where to send their generated data
    if config.output.is_none() {
        return Err(anyhow::anyhow!("Input stage '{}' must have an output", name));
    }

     // Validate that field configuration is appropriate for input stages
    let field_config = extract_field_params(&config.parameters);
    match field_config {
        // Input stages can specify output field names
        FieldConfig::OutputOnly(_) => Ok(()),

        // Input stages can have no specific field requirements
        FieldConfig::None => Ok(()),

        // Input stages shouldn't have input field mappings since they don't consume data
        _ => Err(anyhow::anyhow!("Input stage '{}' should only have output field configuration", name)),
    }
}

/// Validates a pipeline configuration and all its constituent stages.
/// 
/// Pipelines contain multiple processing stages that transform data as it
/// flows through the system. Each stage in a pipeline must be a valid
/// transform stage.
/// 
/// # Arguments
/// 
/// * `name` - The name/identifier of the pipeline
/// * `config` - The pipeline configuration to validate
/// 
/// # Returns
/// 
/// * `Ok(())` - Pipeline and all its stages are valid
/// * `Err(anyhow::Error)` - Pipeline or one of its stages is invalid
/// 
/// # Validation Process
/// 
/// For each stage in the pipeline:
/// 1. Validates stage follows transform stage rules
/// 2. Ensures proper input/output data stream configuration
/// 3. Checks field configuration compatibility
/// 
/// # Example Valid Pipeline
/// 
/// ```toml
/// [pipelines.data_processing]
/// description = "Process sensor data"
/// 
/// [pipelines.data_processing.stages.scale]
/// type = "scale"
/// inputs = ["raw_data"]
/// output = "scaled_data"
/// ```
fn validate_pipeline(name: &str, config: &PipelineConfig) -> anyhow::Result<()> {
    // Validate each stage within the pipeline
    for (stage_name, stage_config) in &config.stages {
        validate_pipeline_stage(name, stage_name, stage_config)
            .map_err(|e| anyhow::anyhow!("Stage '{}' in pipeline '{}': {}", stage_name, name, e))?;
    }
    Ok(())
}

/// Validates an individual stage within a pipeline.
/// 
/// Pipeline stages are transform stages that consume data from input streams,
/// process it, and produce data to output streams. They form the core
/// processing logic of the system.
/// 
/// # Arguments
/// 
/// * `pipeline_name` - The name of the containing pipeline (for error messages)
/// * `stage_name` - The name of the stage being validated
/// * `config` - The stage configuration to validate
/// 
/// # Returns
/// 
/// * `Ok(())` - Stage configuration is valid for a pipeline transform stage
/// * `Err(anyhow::Error)` - Stage configuration violates transform stage rules
/// 
/// # Validation Rules
/// 
/// - **Inputs required**: Transform stages must consume data from somewhere
/// - **At least one input**: Transform stages need data to process
/// - **Output required**: Transform stages must produce data somewhere
/// - **Field config**: Can be any valid field configuration type
/// 
/// # Example Valid Pipeline Stage
/// 
/// ```toml
/// [pipelines.main.stages.filter]
/// type = "lowpass"
/// inputs = ["raw_data", "threshold_config"]
/// output = "filtered_data"
/// parameters = { field_in = "value", field_out = "filtered_value", threshold = 10.0 }
/// ```
fn validate_pipeline_stage(pipeline_name: &str, stage_name: &str, config: &StageConfig) -> anyhow::Result<()> {
    // Transform stages must consume data from input streams
    if config.inputs.is_none() || config.inputs.as_ref().unwrap().is_empty() {
        return Err(anyhow::anyhow!(
            "Pipeline stage '{}.{}' must have at least one input stream configured (what data should it process?)", 
            pipeline_name, 
            stage_name
        ));
    }
    
    // Transform stages must produce data to an output stream
    if config.output.is_none() {
        return Err(anyhow::anyhow!(
            "Pipeline stage '{}.{}' must have an output stream configured (where should processed data go?)", 
            pipeline_name, 
            stage_name
        ));
    }
    
    // Note: Field configuration validation is processor-specific and handled
    // during processor creation, not here at the structural level
    
    Ok(())
}

/// Validates an output stage configuration.
/// 
/// Output stages are data sinks that consume messages from the processing
/// pipeline and handle them appropriately (logging, storage, transmission, etc.).
/// They should only consume data and never produce output.
/// 
/// # Arguments
/// 
/// * `name` - The name/identifier of the output stage
/// * `config` - The stage configuration to validate
/// 
/// # Returns
/// 
/// * `Ok(())` - Stage configuration is valid for an output stage
/// * `Err(anyhow::Error)` - Stage configuration violates output stage rules
/// 
/// # Validation Rules
/// 
/// - **Inputs required**: Output stages must consume data from somewhere
/// - **At least one input**: Output stages need data to process
/// - **No output allowed**: Output stages are terminal, they don't produce data streams
/// - **Field config**: Can be any valid field configuration type
/// 
/// # Example Valid Output Stage
/// 
/// ```toml
/// [outputs.file_logger]
/// type = "log"
/// inputs = ["processed_data", "error_data"]
/// parameters = { destination = "file://logs/output.log", format = "json" }
/// ```
fn validate_output_stage(name: &str, config: &StageConfig) -> anyhow::Result<()> {
    // Output stages must consume data from input streams
    if config.inputs.is_none() || config.inputs.as_ref().unwrap().is_empty() {
        return Err(anyhow::anyhow!(
            "Output stage '{}' must have at least one input stream configured (what data should it consume?)", 
            name
        ));
    }
    
    // Output stages are terminal - they don't produce data streams
    if config.output.is_some() {
        return Err(anyhow::anyhow!(
            "Output stage '{}' should not have an output stream configured (output stages are terminal)", 
            name
        ));
    }
    
    // Note: Field configuration validation is processor-specific and handled
    // during processor creation, not here at the structural level

    Ok(())
}