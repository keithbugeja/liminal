pub mod processor;
pub mod input;
pub mod transform;
pub mod aggregator;
pub mod output;

pub use processor::Processor;
// pub use input::*;
// pub use transform::*;
// pub use aggregator::*;
// pub use output::*;

pub mod factory;
pub use factory::create_processor;
