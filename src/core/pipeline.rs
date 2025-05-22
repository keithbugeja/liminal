use super::registry::ChannelRegistry;
use crate::config::{ConcurrencyType, Config, StageConfig};
use crate::core::channel::PubSubChannel;
use crate::core::message::Message;
use crate::stages::{self, Stage};

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Represents a pipeline consisting of multiple stages.
struct Pipeline {
    name: String,
    description: String,
    stage_names: Vec<String>,
}

/// Manages the creation and connection of stages and pipelines.
pub struct PipelineManager {
    config: Config,
    stages: HashMap<String, Arc<Mutex<Box<dyn Stage>>>>,
    pipelines: HashMap<String, Pipeline>,
    channel_registry: ChannelRegistry<Message>,
}

impl PipelineManager {
    /// Create a new `PipelineManager` with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the pipeline manager.
    ///
    /// # Returns
    ///
    /// A new instance of `PipelineManager`.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            stages: HashMap::new(),
            pipelines: HashMap::new(),
            channel_registry: ChannelRegistry::new(),
        }
    }

    /// Get all stage configurations from the config.
    fn get_all_stage_configs(&self) -> Vec<(String, StageConfig)> {
        let mut all_stages = Vec::new();

        for (stage_name, stage_config) in &self.config.inputs {
            all_stages.push((stage_name.clone(), stage_config.clone()));
        }

        for (_, pipeline_config) in &self.config.pipelines {
            for (stage_name, stage_config) in &pipeline_config.stages {
                all_stages.push((stage_name.clone(), stage_config.clone()));
            }
        }

        for (stage_name, stage_config) in &self.config.outputs {
            all_stages.push((stage_name.clone(), stage_config.clone()));
        }

        all_stages
    }

    /// Check if all inputs for a stage are available in the channel registry.
    fn are_all_inputs_available(
        channel_registry: &ChannelRegistry<Message>,
        stage_config: &StageConfig,
    ) -> Result<bool> {
        Ok(stage_config
            .inputs
            .as_ref()
            .map(|inputs| {
                inputs
                    .iter()
                    .all(|input| channel_registry.get(input).is_some())
            })
            .unwrap_or(true))
    }

    /// Map inputs from the stage configuration to the channel registry.
    async fn map_inputs(
        channel_registry: &mut ChannelRegistry<Message>,
        stage: &Arc<Mutex<Box<dyn Stage>>>,
        stage_config: &StageConfig,
    ) -> Result<()> {
        if let Some(inputs) = &stage_config.inputs {
            for input_name in inputs {
                if let Some(channel) = channel_registry.get(input_name) {
                    let subscriber = channel.subscribe();
                    stage.lock().await.add_input(subscriber);
                } else {
                    return Err(anyhow::anyhow!("Input channel {:?} not found", input_name));
                }
            }
        }

        Ok(())
    }

    /// Create an output channel for the stage if specified in the configuration.
    async fn create_output(
        channel_registry: &mut ChannelRegistry<Message>,
        stage: &Arc<Mutex<Box<dyn Stage>>>,
        stage_config: &StageConfig,
    ) -> Result<()> {
        if let Some(output_name) = &stage_config.output {
            let channel_config = stage_config.channel.clone().unwrap_or_default();
            let channel = channel_registry.get_or_create(
                output_name,
                channel_config.r#type.clone(),
                channel_config.capacity,
            );

            stage.lock().await.add_output(channel.clone());
        }

        Ok(())
    }

    /// Try to connect a stage by mapping its inputs and creating its output.
    async fn try_connect_stage(
        &mut self,
        stage_name: &str,
        stage_config: &StageConfig,
    ) -> Result<()> {
        let stage = self
            .stages
            .get_mut(stage_name)
            .ok_or_else(|| anyhow::anyhow!("Stage not found: {}", stage_name))?;

        if !Self::are_all_inputs_available(&self.channel_registry, stage_config)? {
            return Err(anyhow::anyhow!(
                "Inputs not available for stage: {}",
                stage_name
            ));
        }

        Self::map_inputs(&mut self.channel_registry, stage, stage_config).await?;

        Self::create_output(&mut self.channel_registry, stage, stage_config).await?;

        Ok(())
    }

    /// Resolve deferred stages by checking if their dependencies are met.
    async fn resolve_deferred_stages(
        &mut self,
        mut deferred_stages: Vec<(String, StageConfig)>,
    ) -> Result<()> {
        let mut progress_made = true;

        while !deferred_stages.is_empty() && progress_made {
            progress_made = false;

            let mut new_deferred_stages = Vec::new();

            for (stage_name, stage_config) in deferred_stages.into_iter() {
                if self
                    .try_connect_stage(&stage_name, &stage_config)
                    .await
                    .is_err()
                {
                    new_deferred_stages.push((stage_name, stage_config));
                } else {
                    progress_made = true;
                }
            }

            deferred_stages = new_deferred_stages;
        }

        if !deferred_stages.is_empty() {
            return Err(anyhow::anyhow!(
                "Unmet or circular dependencies detected in stages: {:?}",
                deferred_stages
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            ));
        }

        Ok(())
    }

    /// Connect all stages by resolving their dependencies.
    pub async fn connect_stages(mut self) -> Result<Self> {
        let all_stages = self.get_all_stage_configs();
        let mut deferred_stages = Vec::new();

        for (stage_name, stage_config) in all_stages {
            tracing::info!("Connecting stage [{}]", stage_name);

            if let Err(_) = self.try_connect_stage(&stage_name, &stage_config).await {
                deferred_stages.push((stage_name, stage_config));
            }
        }

        self.resolve_deferred_stages(deferred_stages).await?;

        Ok(self)
    }

    /// Create stages based on the provided stage configurations.
    fn create_stages(
        stage_configs: &HashMap<String, StageConfig>,
    ) -> Result<HashMap<String, Arc<Mutex<Box<dyn Stage>>>>> {
        let mut stages: HashMap<String, Arc<Mutex<Box<dyn Stage>>>> = HashMap::new();

        for (stage_name, stage_config) in stage_configs {
            if let Some(stage) =
                stages::factory::create_stage(&stage_config.r#type, stage_config.clone())
            {
                stages.insert(stage_name.clone(), Arc::new(Mutex::new(stage)));
            } else {
                return Err(anyhow::anyhow!("Failed to create stage: {}", stage_name));
            }
        }

        Ok(stages)
    }

    /// Create pipelines and their stages based on the provided pipeline configurations.
    fn create_pipelines(
        &mut self,
    ) -> Result<(
        HashMap<String, Arc<Mutex<Box<dyn Stage>>>>,
        HashMap<String, Pipeline>,
    )> {
        let mut pipelines = HashMap::new();
        let mut stages = HashMap::new();

        for (pipeline_name, pipeline_config) in &self.config.pipelines {
            let created_stages = Self::create_stages(&pipeline_config.stages)?;

            stages.extend(created_stages);

            let pipeline = Pipeline {
                name: pipeline_name.clone(),
                description: pipeline_config.description.clone(),
                stage_names: pipeline_config.stages.keys().cloned().collect(),
            };

            pipelines.insert(pipeline_name.clone(), pipeline);
        }

        Ok((stages, pipelines))
    }

    /// Build all stages and pipelines based on the provided configuration.
    pub fn build_all(mut self) -> Result<Self> {
        let _ = crate::factory::create_stage_factories();

        // Create input stages
        let input_stages = Self::create_stages(&self.config.inputs)?;
        self.stages.extend(input_stages);

        // Create output stages
        let output_stages = Self::create_stages(&self.config.outputs)?;
        self.stages.extend(output_stages);

        // Create pipelines and pipeline stages
        let (pipeline_stages, pipelines) = self.create_pipelines()?;
        self.stages.extend(pipeline_stages);
        self.pipelines.extend(pipelines);

        Ok(self)
    }

    pub async fn start_all(mut self) -> Result<()> {
        tracing::info!("Starting all stages");
        let all_stages = self.get_all_stage_configs();
        for (stage_name, stage_config) in all_stages {
            if let Some(stage) = self.stages.get_mut(&stage_name) {
                // Initialise the stage
                {
                    let stage_clone = Arc::clone(stage);
                    let mut stage = stage_clone.lock().await;
                    stage.init().await?;
                }

                // Run the stage
                {
                    let stage_clone = Arc::clone(stage);
                    tokio::spawn(async move {
                        let mut stage_lock = stage_clone.lock().await;
                        if let Err(e) = stage_lock.run().await {
                            tracing::error!("Error running stage [{}]: {}", stage_name, e);
                        }
                    });
                }
            }
        }

        // futures::future::pending().await;

        Ok(())
    }
}
