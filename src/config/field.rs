//! Field Configuration Module
//! 
//! This module defines how processors map input fields to output fields.
//! It supports various field transformation patterns commonly used in data processing.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fmt;

/// Configuration for field operations in processors.
/// 
/// Defines how input fields are mapped to output fields during processing.
/// Different variants support different use cases from simple renaming to
/// complex multi-field transformations.
/// 
/// # Examples
/// 
/// ```rust
/// use liminal::config::FieldConfig;
/// 
/// // Simple field transformation
/// let config = FieldConfig::Single {
///     input: "temperature".to_string(),
///     output: "scaled_temp".to_string(),
/// };
/// 
/// // Multiple parallel transformations
/// let config = FieldConfig::Multiple {
///     inputs: vec!["temp".to_string(), "humidity".to_string()],
///     outputs: vec!["scaled_temp".to_string(), "scaled_humidity".to_string()],
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldConfig {
    /// Single field transformation: input field → output field
    /// 
    /// Used when a processor transforms one input field to one output field.
    /// This is the most common case for processors like scale, filter, etc. 
    Single {
        input: String,
        output: String,
    },

    /// Multiple parallel field transformations
    /// 
    /// Processes multiple input fields in parallel, producing corresponding
    /// output fields. The inputs and outputs vectors must have the same length.
    Multiple {
        inputs: Vec<String>,
        outputs: Vec<String>,
    },

    /// Complex field mapping with custom input→output relationships
    /// 
    /// Allows arbitrary mapping between input and output field names.
    /// Useful for processors that need non-parallel field transformations.
    Mapping(HashMap<String, String>),

    /// Output-only configuration for input/source processors
    /// 
    /// Used by processors that generate data (like simulators) and only
    /// need to specify what field name to use for their output.
    OutputOnly (String),

    /// No field configuration required
    /// 
    /// Used by processors that don't need field mapping, such as
    /// logging processors that work with entire messages.
    None,
}

impl FieldConfig {
    /// Creates a single field transformation configuration.
    /// 
    /// # Arguments
    /// * `input` - The input field name
    /// * `output` - The output field name
    /// 
    /// # Returns
    /// A validated `FieldConfig::Single` configuration
    /// 
    /// # Errors
    /// Returns an error if field names are empty
    pub fn single(input: impl Into<String>, output: impl Into<String>) -> anyhow::Result<Self> {
        let config = Self::Single {
            input: input.into(),
            output: output.into(),
        };
        config.validate()?;
        Ok(config)
    }

    /// Creates a multiple field transformation configuration.
    /// 
    /// # Arguments
    /// * `inputs` - Vector of input field names
    /// * `outputs` - Vector of output field names (must match inputs length)
    /// 
    /// # Returns
    /// A validated `FieldConfig::Multiple` configuration
    /// 
    /// # Errors
    /// Returns an error if vectors have different lengths or contain empty names
    pub fn multiple(
        inputs: impl Into<Vec<String>>,
        outputs: impl Into<Vec<String>>,
    ) -> anyhow::Result<Self> {
        let config = Self::Multiple {
            inputs: inputs.into(),
            outputs: outputs.into(),
        };
        config.validate()?;
        Ok(config)
    }

    /// Creates an output-only field configuration.
    /// 
    /// # Arguments
    /// * `field` - The output field name
    /// 
    /// # Returns
    /// A validated `FieldConfig::OutputOnly` configuration
    /// 
    /// # Errors
    /// Returns an error if the field name is empty
    pub fn output_only(field: impl Into<String>) -> anyhow::Result<Self> {
        let config = Self::OutputOnly(field.into());
        config.validate()?;
        Ok(config)
    }    
    
    /// Validates the field configuration for consistency.
    /// 
    /// # Errors
    /// 
    /// Returns an error if:
    /// - Multiple config has mismatched input/output lengths
    /// - Field names are empty
    /// - Multiple config vectors are empty
    /// 
    /// # Example
    /// 
    /// ```rust
    /// let config = FieldConfig::Multiple {
    ///     inputs: vec!["a".to_string()],
    ///     outputs: vec!["b".to_string(), "c".to_string()],
    /// };
    /// assert!(config.validate().is_err()); // Mismatched lengths
    /// ```
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            FieldConfig::Single { input, output } => {
                if input.is_empty() {
                    return Err(anyhow::anyhow!("Input field name cannot be empty"));
                }
                if output.is_empty() {
                    return Err(anyhow::anyhow!("Output field name cannot be empty"));
                }
            }
            FieldConfig::Multiple { inputs, outputs } => {
                if inputs.is_empty() {
                    return Err(anyhow::anyhow!("Multiple field config cannot have empty inputs"));
                }
                if inputs.len() != outputs.len() {
                    return Err(anyhow::anyhow!(
                        "Input fields ({}) and output fields ({}) count mismatch",
                        inputs.len(),
                        outputs.len()
                    ));
                }
                for (i, input) in inputs.iter().enumerate() {
                    if input.is_empty() {
                        return Err(anyhow::anyhow!("Input field {} cannot be empty", i));
                    }
                }
                for (i, output) in outputs.iter().enumerate() {
                    if output.is_empty() {
                        return Err(anyhow::anyhow!("Output field {} cannot be empty", i));
                    }
                }
            }
            FieldConfig::Mapping(map) => {
                if map.is_empty() {
                    return Err(anyhow::anyhow!("Field mapping cannot be empty"));
                }
                for (input, output) in map {
                    if input.is_empty() {
                        return Err(anyhow::anyhow!("Input field name cannot be empty"));
                    }
                    if output.is_empty() {
                        return Err(anyhow::anyhow!("Output field name cannot be empty"));
                    }
                }
            }
            FieldConfig::OutputOnly(field) => {
                if field.is_empty() {
                    return Err(anyhow::anyhow!("Output field name cannot be empty"));
                }
            }
            FieldConfig::None => {} // Always valid
        }
        Ok(())
    }

    /// Checks if this configuration is compatible with a processor type.
    /// 
    /// Different processors expect different field configuration patterns.
    /// This method helps validate configurations at processor creation time.
    /// 
    /// # Arguments
    /// * `processor_type` - The type name of the processor
    /// 
    /// # Returns
    /// `true` if the configuration is compatible, `false` otherwise
    pub fn is_compatible_with_processor(&self, processor_type: &str) -> bool {
        // To make this implementation extensible and as general as possible,
        // I'll look at it as soon as I have ironed out the registration of
        // processors with metadata.
        // todo!()
        true
    }    

    /// Returns all input field names referenced by this configuration.
    /// 
    /// # Returns
    /// A vector of field name references. Empty for output-only configurations.
    pub fn input_fields(&self) -> Vec<&str> {
        match self {
            FieldConfig::Single {input, ..} => vec![input],
            FieldConfig::Multiple {inputs, ..} => inputs.iter().map(|s| s.as_str()).collect(),
            FieldConfig::Mapping(map) => map.keys().map(|s| s.as_str()).collect(),
            FieldConfig::OutputOnly(_) | FieldConfig::None => vec![],
        }
    }

    /// Returns all output field names produced by this configuration.
    /// 
    /// # Returns
    /// A vector of field name references. Empty for no-field configurations.
    pub fn output_fields(&self) -> Vec<&str> {
        match self {
            FieldConfig::Single {output, ..} => vec![output],
            FieldConfig::Multiple {outputs, ..} => outputs.iter().map(|s| s.as_str()).collect(),
            FieldConfig::Mapping(map) => map.values().map(|s| s.as_str()).collect(),
            FieldConfig::OutputOnly(field) => vec![field],
            FieldConfig::None => vec![],
        }
    }

    /// Gets the output field name for a given input field.
    /// 
    /// For configurations that map input fields to output fields, this method
    /// returns the corresponding output field name for the given input.
    /// 
    /// # Arguments
    /// * `input` - The input field name to look up
    /// 
    /// # Returns
    /// * `Some(&str)` - The corresponding output field name
    /// * `None` - No mapping exists for this input field
    /// 
    /// # Example
    /// 
    /// ```rust
    /// let config = FieldConfig::Single {
    ///     input: "temp".to_string(),
    ///     output: "temperature".to_string(),
    /// };
    /// assert_eq!(config.get_output_for_input("temp"), Some("temperature"));
    /// assert_eq!(config.get_output_for_input("other"), None);
    /// ```
    pub fn get_output_for_input(&self, input: &str) -> Option<String> {
        match self {
            FieldConfig::Single {input: i, output} if i == input => Some(output.clone()),
            
            FieldConfig::Multiple { inputs, outputs } => {
                inputs.iter()
                    .position(|i| i == input)
                    .and_then(|index| outputs.get(index).cloned())
            },

            FieldConfig::Mapping(map) => map.get(input).cloned(),
            _ => None,
        }
    }

    /// Checks if this configuration maps any input fields.
    /// 
    /// # Returns
    /// `true` if this configuration processes input fields, `false` for output-only configs.
    pub fn has_inputs(&self) -> bool {
        !self.input_fields().is_empty()
    }

    /// Checks if this configuration produces any output fields.
    /// 
    /// # Returns
    /// `true` if this configuration produces output fields, `false` for no-field configs.
    pub fn has_outputs(&self) -> bool {
        !self.output_fields().is_empty()
    }

    /// Returns the number of field transformations this configuration performs.
    /// 
    /// # Returns
    /// The count of input→output field mappings
    pub fn transformation_count(&self) -> usize {
        match self {
            FieldConfig::Single { .. } => 1,
            FieldConfig::Multiple { inputs, .. } => inputs.len(),
            FieldConfig::Mapping(map) => map.len(),
            FieldConfig::OutputOnly(_) => 1,
            FieldConfig::None => 0,
        }
    }    
}

impl fmt::Display for FieldConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldConfig::Single { input, output } => {
                write!(f, "{} → {}", input, output)
            }
            FieldConfig::Multiple { inputs, outputs } => {
                let mappings: Vec<String> = inputs
                    .iter()
                    .zip(outputs.iter())
                    .map(|(i, o)| format!("{} → {}", i, o))
                    .collect();
                write!(f, "[{}]", mappings.join(", "))
            }
            FieldConfig::Mapping(map) => {
                let mappings: Vec<String> = map
                    .iter()
                    .map(|(i, o)| format!("{} → {}", i, o))
                    .collect();
                write!(f, "{{{}}}", mappings.join(", "))
            }
            FieldConfig::OutputOnly(field) => {
                write!(f, "→ {}", field)
            }
            FieldConfig::None => write!(f, "no fields"),
        }
    }
}