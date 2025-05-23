mod fusion;
mod log;
mod lowpass;
mod scale;
mod simulated;

pub mod factory;
pub mod processor;

pub use factory::create_processor;
pub use factory::create_processor_factories;
pub use processor::Processor;
