use super::schema::*;

pub fn validate_config(config: &Config) -> Result<(), String> {
    // Validate aggregations reference existing input sources
    for (name, aggregation) in &config.aggregations {
        for input in &aggregation.inputs {
            if !config.input_sources.contains_key(input) {
                return Err(format!(
                    "Aggregation '{}' references unknown input source '{}'",
                    name, input
                ));
            }
        }

        if aggregation.inputs.is_empty() {
            return Err(format!(
                "Aggregation '{}' must have at least one input source",
                name
            ));
        }
    }

    // Additional checks

    Ok(())
}
