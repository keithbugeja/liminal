use super::schema::*;

pub fn validate_config(config: &Config) -> Result<(), String> {
    if config.inputs.is_empty() {
        return Err("No input sources defined".into());
    }

    if config.pipelines.is_empty() {
        return Err("No pipeline stages defined".into());
    }

    if config.outputs.is_empty() {
        return Err("No output sources defined".into());
    }

    // for (pipeline_name, pipeline) in &config.pipelines {
    //     let mut available_outputs = config.inputs.keys().cloned().collect::<Vec<String>>();

    //     for (stage_name, stage) in &pipeline.stages {
    //         if let Some(inputs) = &stage.inputs {
    //             for input in inputs {
    //                 if !available_outputs.contains(input) {
    //                     println!("Available outputs: {:?}", available_outputs);
    //                     return Err(format!(
    //                         "Pipeline '{}' stage '{}' references unknown input source '{}'",
    //                         pipeline_name, stage_name, input
    //                     ));
    //                 }
    //             }
    //         }

    //         if let Some(output) = &stage.output {
    //             available_outputs.push(output.clone());
    //         }
    //     }
    // }

    Ok(())
}
