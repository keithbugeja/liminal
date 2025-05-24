use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
pub enum FieldConfig {
    Single {
        input: String,
        output: String,
    },

    Multiple {
        inputs: Vec<String>,
        outputs: Vec<String>,
    },

    Mapping(HashMap<String, String>),

    OutputOnly (String),

    None,
}

impl FieldConfig {
    pub fn input_fields(&self) -> Vec<&str> {
        match self {
            FieldConfig::Single {input, ..} => vec![input],
            FieldConfig::Multiple {inputs, ..} => inputs.iter().map(|s| s.as_str()).collect(),
            FieldConfig::Mapping(map) => map.keys().map(|s| s.as_str()).collect(),
            FieldConfig::OutputOnly(_) | FieldConfig::None => vec![],
        }
    }

    pub fn output_fields(&self) -> Vec<&str> {
        match self {
            FieldConfig::Single {output, ..} => vec![output],
            FieldConfig::Multiple {outputs, ..} => outputs.iter().map(|s| s.as_str()).collect(),
            FieldConfig::Mapping(map) => map.values().map(|s| s.as_str()).collect(),
            FieldConfig::OutputOnly(field) => vec![field],
            FieldConfig::None => vec![],
        }
    }

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
}