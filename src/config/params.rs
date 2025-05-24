//! Parameter Extraction Module
//! 
//! This module provides utilities for extracting and converting processor parameters
//! from the generic `HashMap<String, serde_json::Value>` format used in stage
//! configurations. It handles both scalar parameters and field mapping configurations.
//! 
//! # Parameter Extraction
//! 
//! The `extract_param` function provides type-safe extraction of parameters with
//! fallback to default values when parameters are missing or invalid.
//! 
//! # Field Configuration Extraction
//! 
//! The `extract_field_params` function analyzes parameter maps to determine the
//! appropriate field mapping pattern, supporting multiple configuration styles
//! for different processor types.
//! 
//! # Supported Field Patterns
//! 
//! 1. **Single field transformation**: `field_in` → `field_out`
//! 2. **Output-only**: → `field_out` (for input processors)
//! 3. **Multiple parallel fields**: `fields_in[]` → `fields_out[]`
//! 4. **Custom mapping**: `field_mapping` hash map
//! 5. **No field config**: Default when no field parameters are found
//! 
//! # Example Usage
//! 
//! ```rust
//! use liminal::config::params::{extract_param, extract_field_params};
//! 
//! // Extract scalar parameters with defaults
//! let scale_factor = extract_param(&config.parameters, "scale_factor", 1.0);
//! let timeout = extract_param(&config.parameters, "timeout", 5000_u64);
//! 
//! // Extract field configuration
//! let field_config = extract_field_params(&config.parameters);
//! ```

use crate::config::field::FieldConfig;
use std::collections::HashMap;

/// Extracts a typed parameter from the stage configuration parameters.
/// 
/// This function safely extracts and deserialises parameters from the generic
/// JSON value map, providing type safety and graceful fallback to default values
/// when parameters are missing or cannot be deserialised.
/// 
/// # Type Safety
/// 
/// The function uses Serde's deserialisation to ensure type safety. If the
/// parameter exists but cannot be converted to the target type, the default
/// value is returned and the error is silently handled.
/// 
/// # Arguments
/// 
/// * `params` - Optional parameter map from stage configuration
/// * `key` - The parameter name to extract
/// * `default` - The default value to return if extraction fails
/// 
/// # Returns
/// 
/// The extracted parameter value of type `T`, or the default value if:
/// - The parameter map is `None`
/// - The parameter key doesn't exist
/// - The parameter value cannot be deserialised to type `T`
/// 
/// # Examples
/// 
/// ```rust
/// use std::collections::HashMap;
/// use serde_json::json;
/// 
/// let mut params = HashMap::new();
/// params.insert("scale_factor".to_string(), json!(2.5));
/// params.insert("iterations".to_string(), json!(100));
/// params.insert("enable_logging".to_string(), json!(true));
/// 
/// // Extract with correct types
/// let scale: f64 = extract_param(&Some(params.clone()), "scale_factor", 1.0);
/// assert_eq!(scale, 2.5);
/// 
/// // Extract missing parameter (returns default)
/// let missing: i32 = extract_param(&Some(params.clone()), "missing_key", 42);
/// assert_eq!(missing, 42);
/// 
/// // Extract with type mismatch (returns default)
/// let invalid: String = extract_param(&Some(params), "iterations", "default".to_string());
/// assert_eq!(invalid, "default");
/// ```
/// 
/// # Type Requirements
/// 
/// The type `T` must implement:
/// - `serde::de::DeserializeOwned` - For safe deserialisation from JSON
/// - `Clone` - For returning the default value
/// 
/// # Error Handling
/// 
/// This function uses silent error handling - deserialisation errors are not
/// propagated but result in the default value being returned. For cases where
/// you need to distinguish between missing parameters and invalid values,
/// consider implementing a separate validation function.
pub fn extract_param<T>(
    params: &Option<HashMap<String, serde_json::Value>>,
    key: &str,
    default: T,
) -> T
where
    T: serde::de::DeserializeOwned + Clone,
{
    params
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(default)
}

/// Extracts field mapping configuration from stage parameters.
/// 
/// This function analyzes the parameter map to determine the appropriate field
/// configuration pattern. It tries multiple parameter naming conventions in
/// priority order to support different processor configuration styles.
/// 
/// # Field Configuration Patterns
/// 
/// The function recognizes these patterns in priority order:
/// 
/// ## 1. Single Field Transformation
/// 
/// **Parameters**: `field_in` and `field_out`  
/// **Use case**: Transform one input field to one output field  
/// **Example**: Scale a temperature value
/// 
/// ```toml
/// [stage.parameters]
/// field_in = "temperature"
/// field_out = "scaled_temperature"
/// ```
/// 
/// ## 2. Output-Only Configuration
/// 
/// **Parameters**: `field_out` only  
/// **Use case**: Input processors that generate data  
/// **Example**: Simulated data source
/// 
/// ```toml
/// [stage.parameters]
/// field_out = "generated_data"
/// ```
/// 
/// ## 3. Multiple Parallel Fields
/// 
/// **Parameters**: `fields_in` and `fields_out` arrays  
/// **Use case**: Apply same transformation to multiple fields  
/// **Example**: Scale multiple sensor readings
/// 
/// ```toml
/// [stage.parameters]
/// fields_in = ["temp", "humidity", "pressure"]
/// fields_out = ["scaled_temp", "scaled_humidity", "scaled_pressure"]
/// ```
/// 
/// **Note**: Arrays must have the same length, mismatched lengths are ignored with a warning.
/// 
/// ## 4. Custom Field Mapping
/// 
/// **Parameters**: `field_mapping` hash map  
/// **Use case**: Complex field transformations with custom mappings  
/// **Example**: Rename and restructure fields
/// 
/// ```toml
/// [stage.parameters]
/// field_mapping = { "old_name" = "new_name", "sensor_1" = "temperature" }
/// ```
/// 
/// ## 5. No Field Configuration
/// 
/// **Default**: When no field parameters are found  
/// **Use case**: Processors that work with entire messages  
/// **Example**: Logging processors
/// 
/// # Arguments
/// 
/// * `params` - Optional parameter map from stage configuration
/// 
/// # Returns
/// 
/// A `FieldConfig` enum variant representing the detected field configuration pattern.
/// Returns `FieldConfig::None` if no field configuration is detected.
/// 
/// # Error Handling
/// 
/// - **Missing parameters**: Returns `FieldConfig::None`
/// - **Invalid JSON types**: Parameters are ignored, continues to next pattern
/// - **Length mismatch**: Arrays with different lengths are ignored with a warning
/// 
/// # Examples
/// 
/// ```rust
/// use std::collections::HashMap;
/// use serde_json::json;
/// use liminal::config::field::FieldConfig;
/// 
/// // Single field configuration
/// let mut params = HashMap::new();
/// params.insert("field_in".to_string(), json!("input"));
/// params.insert("field_out".to_string(), json!("output"));
/// 
/// let config = extract_field_params(&Some(params));
/// match config {
///     FieldConfig::Single { input, output } => {
///         assert_eq!(input, "input");
///         assert_eq!(output, "output");
///     },
///     _ => panic!("Expected single field config"),
/// }
/// ```
/// 
/// # Performance Notes
/// 
/// This function performs multiple parameter extractions in sequence. For performance-critical
/// applications, consider caching the result rather than calling this function repeatedly.
pub fn extract_field_params(params: &Option<HashMap<String, serde_json::Value>>) -> FieldConfig {
    
    // Pattern 1 : Single field (field_in -> field_out)
    if let(Some(field_in), Some(field_out)) = (
        extract_param(params,"field_in", None::<String>),
        extract_param(params,"field_out", None::<String>)
    ) {
        return FieldConfig::Single {
            input: field_in,
            output: field_out,
        };
    }

    // Pattern 1b: Single output field ( -> field_out)
    if let Some(field_out) = extract_param(params,"field_out", None::<String>) {
        return FieldConfig::OutputOnly(field_out);
    }

    // Pattern 2 : Multiple fields (field_in[i] -> field_out[i])
    if let (Some(fields_in), Some(fields_out)) = (
        extract_param(params, "fields_in", None::<Vec<String>>),
        extract_param(params, "fields_out", None::<Vec<String>>)
    ) {
        if fields_in.len() == fields_out.len() {
            return FieldConfig::Multiple { 
                inputs: fields_in, 
                outputs: fields_out 
            };
        } else {
            tracing::warn!("fields_in and fields_out have different lengths, ignoring field config");
        }
    }

    // Pattern 3 : Mapping (*field_in[i] -> *field_out[i])
    if let Some(mapping) = extract_param(params, "field_mapping", None::<HashMap<String, String>>) {
        return FieldConfig::Mapping(mapping);
    }

    FieldConfig::None
}