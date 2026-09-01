use super::runtime_event::RuntimeEvent;
use std::sync::Arc;

pub const RUNTIME_EVENT_PREFIX: &str = "LIMINAL_EVENT ";

pub trait RuntimeObserver: Send + Sync {
    fn emit(&self, event: RuntimeEvent);
}

pub type SharedRuntimeObserver = Arc<dyn RuntimeObserver>;

#[derive(Default)]
pub struct NoopRuntimeObserver;

impl RuntimeObserver for NoopRuntimeObserver {
    fn emit(&self, _event: RuntimeEvent) {}
}

pub struct JsonlRuntimeObserver;

impl RuntimeObserver for JsonlRuntimeObserver {
    fn emit(&self, event: RuntimeEvent) {
        match serde_json::to_string(&event) {
            Ok(json) => eprintln!("{RUNTIME_EVENT_PREFIX}{json}"),
            Err(error) => tracing::error!("Failed to serialize runtime event: {}", error),
        }
    }
}

pub fn noop_runtime_observer() -> SharedRuntimeObserver {
    Arc::new(NoopRuntimeObserver)
}

pub fn jsonl_runtime_observer() -> SharedRuntimeObserver {
    Arc::new(JsonlRuntimeObserver)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct RecordingRuntimeObserver {
        events: Mutex<Vec<RuntimeEvent>>,
    }

    impl RecordingRuntimeObserver {
        pub fn events(&self) -> Vec<RuntimeEvent> {
            self.events
                .lock()
                .expect("recording observer lock is not poisoned")
                .clone()
        }
    }

    impl RuntimeObserver for RecordingRuntimeObserver {
        fn emit(&self, event: RuntimeEvent) {
            self.events
                .lock()
                .expect("recording observer lock is not poisoned")
                .push(event);
        }
    }
}
