pub mod common;
pub mod factory;
pub mod processor;

pub mod aggregator;
pub mod input;
pub mod output;
pub mod transform;

pub use processor::Processor;
// pub use input::*;
// pub use transform::*;
// pub use aggregator::*;
// pub use output::*;

pub use factory::create_processor;
