use super::types::*;
use super::params::extract_field_params;
use super::field::FieldConfig;

/// Validate the entire configuration
pub fn validate_config(config: &Config) -> anyhow::Result<()> {

    // Validate inputs
    for (name, stage_config) in &config.inputs {
        validate_input_stage(name, stage_config)?;
    }

    // Validate pipelines
    for (name, pipeline_config) in &config.pipelines {
        validate_pipeline(name, pipeline_config)?;
    }
    
    // Validate outputs
    for (name, stage_config) in &config.outputs {
        validate_output_stage(name, stage_config)?;
    }

    Ok(())
}

fn validate_input_stage(name: &str, config: &StageConfig) -> anyhow::Result<()> {
    // Input stages should have output but no inputs
    if config.inputs.is_some() {
        return Err(anyhow::anyhow!("Input stage '{}' should not have inputs", name));
    }
    if config.output.is_none() {
        return Err(anyhow::anyhow!("Input stage '{}' must have an output", name));
    }

    // Validate field configuration
    let field_config = extract_field_params(&config.parameters);
    match field_config {
        FieldConfig::OutputOnly(_) => Ok(()),
        FieldConfig::None => Ok(()),
        _ => Err(anyhow::anyhow!("Input stage '{}' should only have output field configuration", name)),
    }
}

fn validate_pipeline(name: &str, config: &PipelineConfig) -> anyhow::Result<()> {
    for (stage_name, stage_config) in &config.stages {
        validate_pipeline_stage(name, stage_name, stage_config)?;
    }
    Ok(())
}

fn validate_pipeline_stage(pipeline_name: &str, stage_name: &str, config: &StageConfig) -> anyhow::Result<()> {
    // Pipeline stages should have both inputs and output
    if config.inputs.is_none() || config.inputs.as_ref().unwrap().is_empty() {
        return Err(anyhow::anyhow!("Pipeline stage '{}.{}' must have inputs", pipeline_name, stage_name));
    }
    if config.output.is_none() {
        return Err(anyhow::anyhow!("Pipeline stage '{}.{}' must have an output", pipeline_name, stage_name));
    }
    Ok(())
}

fn validate_output_stage(name: &str, config: &StageConfig) -> anyhow::Result<()> {
    // Output stages should have inputs but no output
    if config.inputs.is_none() || config.inputs.as_ref().unwrap().is_empty() {
        return Err(anyhow::anyhow!("Output stage '{}' must have inputs", name));
    }
    if config.output.is_some() {
        return Err(anyhow::anyhow!("Output stage '{}' should not have an output", name));
    }
    Ok(())
}