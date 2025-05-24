//! Configuration Traits Module
//! 
//! This module defines traits for processor-specific configuration handling.
//! It provides a standardized interface for processors to extract and validate
//! their configuration parameters from the generic `StageConfig` structure.
//! 
//! # Design Pattern
//! 
//! The `ProcessorConfig` trait enables type-safe configuration extraction:
//! 1. Generic `StageConfig` is loaded from TOML files
//! 2. Each processor defines its own config struct implementing `ProcessorConfig`
//! 3. Processor-specific config is extracted and validated during processor creation
//! 
//! # Example Usage
//! 
//! ```rust
//! use liminal::config::traits::ProcessorConfig;
//! use liminal::config::StageConfig;
//! 
//! #[derive(Debug)]
//! struct ScaleProcessorConfig {
//!     scale_factor: f64,
//!     field_in: String,
//!     field_out: String,
//! }
//! 
//! impl ProcessorConfig for ScaleProcessorConfig {
//!     fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
//!         // Extract and validate processor-specific parameters
//!         let scale_factor = extract_param(&config.parameters, "scale_factor", 1.0)?;
//!         let field_in = extract_param(&config.parameters, "field_in", None::<String>)?
//!             .ok_or_else(|| anyhow::anyhow!("field_in parameter is required"))?;
//!         let field_out = extract_param(&config.parameters, "field_out", None::<String>)?
//!             .ok_or_else(|| anyhow::anyhow!("field_out parameter is required"))?;
//!         
//!         Ok(Self { scale_factor, field_in, field_out })
//!     }
//!     
//!     fn validate(&self) -> anyhow::Result<()> {
//!         if self.scale_factor <= 0.0 {
//!             return Err(anyhow::anyhow!("scale_factor must be positive"));
//!         }
//!         Ok(())
//!     }
//! }
//! ```

use crate::config::types::StageConfig;

/// Trait for processor-specific configuration extraction and validation.
/// 
/// This trait provides a standardised way for processors to convert the generic
/// `StageConfig` into their own strongly-typed configuration structure. It enables
/// type-safe parameter extraction and processor-specific validation logic.
/// 
/// # Type Safety Benefits
/// 
/// - **Compile-time guarantees**: Processor configs are validated at the type level
/// - **Clear interfaces**: Each processor's required parameters are explicit
/// - **Validation separation**: Generic config validation vs processor-specific validation
/// - **Error handling**: Detailed error messages for missing or invalid parameters
/// 
/// # Implementation Guidelines
/// 
/// ## Required Parameters
/// Use `extract_param` with `None` default and `ok_or_else` for required params:
/// ```rust
/// let scale_factor = extract_param(&config.parameters, "scale_factor", None::<f64>)?
///     .ok_or_else(|| anyhow::anyhow!("scale_factor parameter is required"))?;
/// ```
/// 
/// ## Optional Parameters
/// Use `extract_param` with a sensible default value:
/// ```rust
/// let timeout = extract_param(&config.parameters, "timeout", 5000_u64)?;
/// ```
/// 
/// ## Field Configuration
/// Use `extract_field_params` to get field mapping configuration:
/// ```rust
/// use crate::config::field::FieldConfig;
/// use crate::config::params::extract_field_params;
/// 
/// let field_config = extract_field_params(&config.parameters);
/// match field_config {
///     FieldConfig::Single { input, output } => {
///         // Handle single field transformation
///     },
///     FieldConfig::Multiple { inputs, outputs } => {
///         // Handle multiple field transformations
///     },
///     _ => return Err(anyhow::anyhow!("Invalid field configuration for this processor")),
/// }
/// ```
/// 
/// ## Validation
/// Implement `validate()` to check parameter combinations and constraints:
/// ```rust
/// fn validate(&self) -> anyhow::Result<()> {
///     if self.min_value >= self.max_value {
///         return Err(anyhow::anyhow!("min_value must be less than max_value"));
///     }
///     Ok(())
/// }
/// ```
/// 
/// # Error Handling
/// 
/// Both methods should return detailed errors that help users fix their configuration:
/// - Include parameter names in error messages
/// - Suggest valid ranges or values where appropriate
/// - Explain the purpose of required parameters
/// 
/// # Example Implementation
/// 
/// ```rust
/// use crate::config::field::FieldConfig;
/// use crate::config::params::{extract_param, extract_field_params};
/// 
/// #[derive(Debug)]
/// struct ScaleProcessorConfig {
///     scale_factor: f64,
///     field_config: FieldConfig,
/// }
/// 
/// impl ProcessorConfig for ScaleProcessorConfig {
///     fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
///         // Extract scalar parameters
///         let scale_factor = extract_param(&config.parameters, "scale_factor", 1.0)?;
///         
///         // Extract field configuration
///         let field_config = extract_field_params(&config.parameters);
///         
///         // Validate field config is appropriate for this processor
///         match &field_config {
///             FieldConfig::Single { .. } | FieldConfig::Multiple { .. } => {
///                 // Scale processor supports these field patterns
///             },
///             _ => return Err(anyhow::anyhow!("Scale processor requires field mapping configuration")),
///         }
///         
///         let config = Self { scale_factor, field_config };
///         config.validate()?;
///         Ok(config)
///     }
///     
///     fn validate(&self) -> anyhow::Result<()> {
///         if self.scale_factor <= 0.0 {
///             return Err(anyhow::anyhow!("scale_factor must be positive"));
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait ProcessorConfig: Sized {   
    /// Extracts processor-specific configuration from a generic stage configuration.
    /// 
    /// This method should parse the `parameters` field of the `StageConfig` and
    /// extract all processor-specific settings into a strongly-typed configuration
    /// structure. It should validate that all required parameters are present and
    /// have reasonable values.
    /// 
    /// # Arguments
    /// 
    /// * `config` - The generic stage configuration containing processor parameters
    /// 
    /// # Returns
    /// 
    /// * `Ok(Self)` - Successfully extracted and validated processor configuration
    /// * `Err(anyhow::Error)` - Missing required parameters or invalid values
    /// 
    /// # Error Guidelines
    /// 
    /// Return errors that help users fix their configuration:
    /// - "Parameter 'scale_factor' is required for scale processor"
    /// - "Parameter 'threshold' must be positive (got -5.0)"
    /// - "Parameter 'field_in' cannot be empty"
    /// 
    /// # Implementation Note
    /// 
    /// This method should typically call `validate()` at the end to ensure
    /// the extracted configuration is consistent and valid.
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self>;
    
    /// Validates the processor configuration for internal consistency.
    /// 
    /// This method should check that parameter combinations make sense and
    /// that all constraints are satisfied. It's called automatically by
    /// `from_stage_config()` but can also be called independently.
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - Configuration is valid and ready for use
    /// * `Err(anyhow::Error)` - Configuration has consistency issues
    /// 
    /// # Default Implementation
    /// 
    /// The default implementation returns `Ok(())`, which is appropriate for
    /// processors that don't need complex validation logic. Override this
    /// method if your processor needs to validate parameter relationships.
    /// 
    /// # Example Validations
    /// 
    /// - Numeric ranges: "scale_factor must be positive"
    /// - Field relationships: "field_in and field_out must be different"
    /// - Parameter combinations: "cannot specify both 'value' and 'range'"
    /// - Resource constraints: "buffer_size must be a power of 2"
    fn validate(&self) -> anyhow::Result<()> { Ok(()) }
}