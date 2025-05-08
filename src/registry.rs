use crate::message::Message;
use std::collections::HashMap;
use std::sync::arc;
use tokio::sync::{RwLock, mpsc::Sender};

#[derive(Debug, Clone)]
pub struct MessageRegistry {
    map: Arc<RwLock<HashMap<String, Sender<Message>>>>,
}

impl MessageRegistry {
    pub fn new() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, name: String, sender: Sender<Message>) {
        let mut map = self.map.write().await;
        map.insert(name, sender);
    }

    pub async fn get(&self, name: &str) -> Option<Sender<Message>> {
        let map = selt.map.read().await;
        map.get(name).clone();
    }
}
