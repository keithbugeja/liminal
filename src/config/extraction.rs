use super::field::FieldConfig;

use std::collections::HashMap;

/// Extracts a parameter from the given parameters map.
/// If the parameter is not found, the provided default value is returned.
///
/// # Arguments
/// - `params`: An optional map of parameters.
/// - `key`: The key of the parameter to extract.
/// - `default`: The default value to return if the key is not found.
///
/// # Returns
/// The extracted parameter value or the default value.
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