use crate::config::extract_param;
use anyhow::Result;
use rumqttc::{MqttOptions, QoS};
use std::collections::HashMap;

/// Common MQTT configuration shared between input and output processors
#[derive(Debug, Clone)]
pub struct MqttConnectionConfig {
    pub broker_url: String,
    pub client_id: Option<String>,
    pub qos: u8,
    pub clean_session: bool,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl MqttConnectionConfig {
    /// Extract common MQTT connection parameters from stage config
    pub fn from_parameters(
        parameters: &Option<HashMap<String, serde_json::Value>>,
        _default_client_prefix: &str,
    ) -> Self {
        let broker_url = extract_param(parameters, "broker_url", "mqtt://localhost:1883".to_string());
        let client_id = extract_param(parameters, "client_id", None);
        let qos = extract_param(parameters, "qos", 0);
        let clean_session = extract_param(parameters, "clean_session", true);
        let username = extract_param(parameters, "username", None);
        let password = extract_param(parameters, "password", None);

        Self {
            broker_url,
            client_id,
            qos,
            clean_session,
            username,
            password,
        }
    }

    /// Validate common MQTT connection parameters
    pub fn validate(&self) -> Result<()> {
        if self.qos > 2 {
            return Err(anyhow::anyhow!("QoS must be between 0 and 2"));
        }
        if self.broker_url.is_empty() {
            return Err(anyhow::anyhow!("Broker URL cannot be empty"));
        }
        Ok(())
    }

    /// Parse broker URL into host and port
    pub fn parse_broker_url(&self) -> Result<(String, u16)> {
        let url = &self.broker_url;
        let clean_url = if url.starts_with("mqtt://") { &url[7..] } else { url };

        if let Some(colon_pos) = clean_url.find(':') {
            let host = clean_url[..colon_pos].to_string();
            let port = clean_url[colon_pos + 1..].parse::<u16>()
                .map_err(|_| anyhow::anyhow!("Invalid port in broker URL: {}", url))?;
            Ok((host, port))
        } else {
            Ok((clean_url.to_string(), 1883))
        }
    }

    /// Convert u8 QoS to rumqttc QoS enum
    pub fn qos(&self) -> QoS {
        match self.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::AtMostOnce,
        }
    }

    /// Create MqttOptions from the configuration
    pub fn create_mqtt_options(&self, default_client_prefix: &str) -> Result<MqttOptions> {
        let (host, port) = self.parse_broker_url()?;

        let client_id = self
            .client_id
            .clone()
            .unwrap_or_else(|| format!("{}_{}", default_client_prefix, uuid::Uuid::new_v4()));

        let mut mqttoptions = MqttOptions::new(&client_id, host, port);
        mqttoptions.set_clean_session(self.clean_session);

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            mqttoptions.set_credentials(username, password);
        }

        Ok(mqttoptions)
    }
}
