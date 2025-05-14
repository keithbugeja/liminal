use crate::config::Config;
use crate::stages::{self, Stage};
use anyhow::Result;
use std::collections::HashMap;

struct Pipeline {
    name: String,
    description: String,
    stage_names: Vec<String>,
}

pub struct PipelineManager {
    config: Config,
    stages: HashMap<String, Box<dyn Stage>>,
    pipelines: HashMap<String, Pipeline>,
}

impl PipelineManager {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            stages: HashMap::new(),
            pipelines: HashMap::new(),
        }
    }

    pub fn build_all(mut self) -> Result<Self> {
        let _ = crate::factory::create_stage_factories();

        // Create input stages
        for (stage_name, stage_cfg) in &self.config.inputs {
            if let Some(stage) = stages::factory::create_stage(&stage_cfg.r#type, stage_cfg.clone())
            {
                self.stages.insert(stage_name.clone(), stage);
            }
        }

        // Create pipeline stages
        for (pipeline_name, pipeline_cfg) in &self.config.pipelines {
            for (stage_name, stage_cfg) in &pipeline_cfg.stages {
                if let Some(stage) =
                    stages::factory::create_stage(&stage_cfg.r#type, stage_cfg.clone())
                {
                    self.stages.insert(stage_name.clone(), stage);
                }
            }

            // Create a pipeline object and add it to the manager
            let pipeline = Pipeline {
                name: pipeline_name.clone(),
                description: pipeline_cfg.description.clone(),
                stage_names: pipeline_cfg.stages.keys().cloned().collect(),
            };

            self.pipelines.insert(pipeline_name.clone(), pipeline);
        }

        for (stage_name, stage_cfg) in &self.config.outputs {
            if let Some(stage) = stages::factory::create_stage(&stage_cfg.r#type, stage_cfg.clone())
            {
                self.stages.insert(stage_name.clone(), stage);
            }
        }

        // let factories = crate::factory::all_stage_factories();

        // for stage_cfg in &self.config.stages {
        //     let factory = factories
        //         .get(&stage_cfg.r#type)
        //         .ok_or_else(|| anyhow::anyhow!("Unknown stage type: {}", stage_cfg.r#type))?;
        //     let stage = function(stage_cfg);
        //     self.stages.insert(stage_cfg.name.clone(), stage);
        // }

        // for stage_cfg in &self.config.stages {
        //     if let Some(output_name) = &stage_cfg.output {
        //         let kind = stage_cfg
        //             .channel
        //             .as_ref()
        //             .map(|c| c.r#type.clone())
        //             .unwrap_or(ChannelType::Broadcast);

        //         let cap = stage_cfg
        //             .channel
        //             .as_ref()
        //             .map(|c| c.capacity)
        //             .unwrap_or(128);
        //         let channel = Channel::new(kind, cap);
        //         let sender = channel.sender();
        //         self.tx_registry.insert(output_name.clone(), sender);
        //     }
        // }

        // for stage_cfg in &self.config.stages {
        //     let stage = self.stages.get_mut(&stage_cfg.name).expect("just inserted");
        //     for upstream in &stage_cfg.inputs {
        //         let sender = self
        //             .tx_registry
        //             .get(upstream)
        //             .ok_or_else(|| anyhow!("No channel for '{}'", upstream))?;
        //         let receiver = sender.receiver();
        //         stage.attach_input(receiver);
        //     }
        // }

        Ok(self)
    }

    pub async fn start_all(self) -> Result<()> {
        // futures::future::pending().await;
        Ok(())
    }
}
