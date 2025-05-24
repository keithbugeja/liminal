use super::channel::{PubSubChannel, Subscriber};
use super::message::Message;

use std::collections::HashMap;
use std::sync::Arc;

pub struct ProcessingContext {
    pub stage_name: String,
    pub inputs: HashMap<String, Subscriber<Message>>,
    pub output: Option<OutputInfo>,
    pub metadata: HashMap<String, String>,
}

pub struct OutputInfo {
    pub channel: Arc<dyn PubSubChannel<Message>>,
    pub name: String,
}

impl ProcessingContext {
    pub fn new(stage_name: String) -> Self {
        Self {
            stage_name,
            inputs: HashMap::new(),
            output: None,
            metadata: HashMap::new(),
        }
    }

    pub fn attach_output(&mut self, name: String, channel: Arc<dyn PubSubChannel<Message>>) {
        self.output = Some(OutputInfo { channel, name });
    }

    pub fn add_input(&mut self, name: String, subscriber: Subscriber<Message>) {
        self.inputs.insert(name, subscriber);
    }
}