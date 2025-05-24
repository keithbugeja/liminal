use super::StageConfig;

pub trait ProcessorConfig: Sized {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self>;
    fn validate(&self) -> anyhow::Result<()> { Ok(()) }
}