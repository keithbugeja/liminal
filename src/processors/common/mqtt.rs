use crate::config::{defaulted_param, optional_param};
use anyhow::Result;
use rumqttc::{Event, EventLoop, MqttOptions, Packet, QoS};
use std::collections::HashMap;
use tokio::time::{Duration, Instant, timeout};

const MQTT_READY_TIMEOUT: Duration = Duration::from_secs(2);

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
    ) -> Result<Self> {
        let broker_url = defaulted_param(
            parameters,
            "broker_url",
            "mqtt://localhost:1883".to_string(),
        )?;
        let client_id = optional_param(parameters, "client_id")?;
        let qos = defaulted_param(parameters, "qos", 0)?;
        let clean_session = defaulted_param(parameters, "clean_session", true)?;
        let username = optional_param(parameters, "username")?;
        let password = optional_param(parameters, "password")?;

        Ok(Self {
            broker_url,
            client_id,
            qos,
            clean_session,
            username,
            password,
        })
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
        let clean_url = if url.starts_with("mqtt://") {
            &url[7..]
        } else {
            url
        };

        if let Some(colon_pos) = clean_url.find(':') {
            let host = clean_url[..colon_pos].to_string();
            let port = clean_url[colon_pos + 1..]
                .parse::<u16>()
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

    pub async fn wait_for_connection_ack(&self, eventloop: &mut EventLoop) -> Result<()> {
        wait_for_mqtt_packet(&self.broker_url, eventloop, "CONNACK", |event| {
            matches!(event, Event::Incoming(Packet::ConnAck(_)))
        })
        .await
    }

    pub async fn wait_for_subscription_acks(
        &self,
        eventloop: &mut EventLoop,
        expected_acks: usize,
    ) -> Result<()> {
        for _ in 0..expected_acks {
            wait_for_mqtt_packet(&self.broker_url, eventloop, "SUBACK", |event| {
                matches!(event, Event::Incoming(Packet::SubAck(_)))
            })
            .await?;
        }

        Ok(())
    }
}

async fn wait_for_mqtt_packet(
    broker_url: &str,
    eventloop: &mut EventLoop,
    label: &str,
    is_expected: impl Fn(&Event) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + MQTT_READY_TIMEOUT;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(anyhow::anyhow!(
                "MQTT broker '{}' did not send {} within {:?}",
                broker_url,
                label,
                MQTT_READY_TIMEOUT
            ));
        }

        let event = timeout(remaining, eventloop.poll())
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "MQTT broker '{}' did not send {} within {:?}",
                    broker_url,
                    label,
                    MQTT_READY_TIMEOUT
                )
            })?
            .map_err(|error| {
                anyhow::anyhow!(
                    "MQTT broker '{}' failed before {}: {}",
                    broker_url,
                    label,
                    error
                )
            })?;

        if is_expected(&event) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mqtt_qos_type_mismatch_is_rejected() {
        let parameters = Some(HashMap::from([("qos".to_string(), json!("at_least_once"))]));
        let error = MqttConnectionConfig::from_parameters(&parameters, "test")
            .expect_err("qos type mismatch is rejected");

        assert!(error.to_string().contains("qos"));
    }

    #[test]
    fn mqtt_qos_range_is_rejected_by_validation() {
        let parameters = Some(HashMap::from([("qos".to_string(), json!(3))]));
        let config =
            MqttConnectionConfig::from_parameters(&parameters, "test").expect("qos deserializes");
        let error = config.validate().expect_err("qos range is rejected");

        assert!(error.to_string().contains("QoS"));
    }

    #[tokio::test]
    async fn mqtt_connection_ack_reports_refused_connection() {
        let config = MqttConnectionConfig {
            broker_url: "mqtt://127.0.0.1:0".to_string(),
            client_id: None,
            qos: 0,
            clean_session: true,
            username: None,
            password: None,
        };
        let mqttoptions = config
            .create_mqtt_options("test")
            .expect("MQTT options can be created");
        let (_client, mut eventloop) = rumqttc::AsyncClient::new(mqttoptions, 10);

        let error = config
            .wait_for_connection_ack(&mut eventloop)
            .await
            .expect_err("port zero is not a reachable MQTT broker");

        assert!(error.to_string().contains("MQTT broker"));
    }
}
