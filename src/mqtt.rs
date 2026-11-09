use anyhow::{Context, Result};
use paho_mqtt::{Client, ConnectOptionsBuilder, CreateOptionsBuilder};
use serde::Deserialize;
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Deserialize, Clone)]
pub struct MqttConfig {
    pub uri: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "MqttConfig::default_topic")]
    pub topic: String,
    #[serde(default = "MqttConfig::default_qos")]
    pub qos: i32,
    #[serde(default = "MqttConfig::default_client_id")]
    pub client_id: String,
    #[serde(
        default = "MqttConfig::default_operation_timeout",
        with = "humantime_serde"
    )]
    pub operation_timeout: Duration,
    #[serde(
        default = "MqttConfig::default_keep_alive_interval",
        with = "humantime_serde"
    )]
    pub keep_alive_interval: Duration,
    #[serde(
        default = "MqttConfig::default_auto_reconnect_interval_min",
        with = "humantime_serde"
    )]
    pub auto_reconnect_interval_min: Duration,
    #[serde(
        default = "MqttConfig::default_auto_reconnect_interval_max",
        with = "humantime_serde"
    )]
    pub auto_reconnect_interval_max: Duration,
}

impl fmt::Debug for MqttConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MqttConfig")
            .field("uri", &self.uri)
            .field("username", &self.username)
            .field(
                "password",
                &self.password.as_ref().map(|_| "********"),
            )
            .field("topic", &self.topic)
            .field("qos", &self.qos)
            .field("client_id", &self.client_id)
            .field("operation_timeout", &self.operation_timeout)
            .field("keep_alive_interval", &self.keep_alive_interval)
            .field("auto_reconnect_interval_min", &self.auto_reconnect_interval_min)
            .field("auto_reconnect_interval_max", &self.auto_reconnect_interval_max)
            .finish()
    }
}

impl MqttConfig {
    fn default_topic() -> String {
        "abl-evcc".into()
    }

    fn default_qos() -> i32 {
        0
    }

    fn generate_random_string(len: usize) -> String {
        use rand::distr::{Alphanumeric, SampleString};
        Alphanumeric.sample_string(&mut rand::rng(), len)
    }

    fn default_client_id() -> String {
        format!("abl-evcc-{}", Self::generate_random_string(8))
    }

    fn default_operation_timeout() -> Duration {
        Duration::from_secs(10)
    }

    fn default_keep_alive_interval() -> Duration {
        Duration::from_secs(30)
    }

    fn default_auto_reconnect_interval_min() -> Duration {
        Duration::from_secs(1)
    }

    fn default_auto_reconnect_interval_max() -> Duration {
        Duration::from_secs(30)
    }

    pub const DEFAULT_CONFIG_FILE: &str = "mqtt.yaml";

    pub fn load(config_file_path: &str) -> Result<Self> {
        log::debug!("Loading config file from {:?}", config_file_path);
        let config_file = std::fs::File::open(config_file_path)
            .with_context(|| format!("Cannot open MQTT config file {:?}", config_file_path))?;
        let mut config: Self = yaml_serde::from_reader(&config_file)
            .with_context(|| format!("Cannot read MQTT config from file: {:?}", config_file_path))?;

        // Override settings with environment variables if set
        if let Ok(env_uri) = std::env::var("MQTT_URI") {
            config.uri = env_uri;
        }
        if let Ok(env_user) = std::env::var("MQTT_USERNAME") {
            config.username = Some(env_user);
        }
        if let Ok(env_pass) = std::env::var("MQTT_PASSWORD") {
            config.password = Some(env_pass);
        }
        if let Ok(env_topic) = std::env::var("MQTT_TOPIC") {
            config.topic = env_topic;
        }
        if let Ok(env_client_id) = std::env::var("MQTT_CLIENT_ID") {
            config.client_id = env_client_id;
        }

        Ok(config)
    }

    pub fn create_client(&self) -> Result<Client> {
        let create_opts = CreateOptionsBuilder::new()
            .server_uri(&self.uri)
            .client_id(&self.client_id)
            .persistence(None)
            .finalize();

        let mut client = Client::new(create_opts)
            .with_context(|| format!("Error creating MQTT client for server: {}", self.uri))?;

        client.set_timeout(self.operation_timeout);

        let mut conn_builder = ConnectOptionsBuilder::new();
        conn_builder
            .keep_alive_interval(self.keep_alive_interval)
            .clean_session(true)
            .automatic_reconnect(
                self.auto_reconnect_interval_min,
                self.auto_reconnect_interval_max,
            );

        if let Some(user_name_str) = &self.username {
            conn_builder.user_name(user_name_str.as_str());
        }
        if let Some(password_str) = &self.password {
            conn_builder.password(password_str.as_str());
        }
        let conn_opts = conn_builder.finalize();

        log::info!(
            "Attempting to connect to MQTT broker: {} with client_id: {}",
            self.uri,
            self.client_id
        );

        let max_retries = 5;
        let mut retry_count = 0;
        let retry_delay = Duration::from_secs(2);

        loop {
            match client.connect(conn_opts.clone()) {
                Ok(_) => {
                    log::info!("Connected to MQTT broker.");
                    break;
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        return Err(e).with_context(|| {
                            format!("Failed to connect to MQTT broker after {max_retries} attempts")
                        });
                    }
                    log::warn!(
                        "Failed to connect to MQTT broker ({e}). Retrying in {retry_delay:?} ({retry_count}/{max_retries})..."
                    );
                    std::thread::sleep(retry_delay);
                }
            }
        }

        Ok(client)
    }
}

pub struct MqttPublisher {
    client: Client,
    topic: String,
    qos: i32,
    was_disconnected: AtomicBool,
}

impl MqttPublisher {
    pub fn new(config_file: &str) -> Result<Self> {
        let config = MqttConfig::load(config_file)?;
        let client = config.create_client()?;
        Ok(Self {
            client,
            topic: config.topic,
            qos: config.qos,
            was_disconnected: AtomicBool::new(false),
        })
    }

    pub fn publish_values(
        &self,
        values: &abl_evcc_lib::tokio_common::AllValues,
        no_json: bool,
    ) -> Result<()> {
        if !self.client.is_connected() {
            if !self.was_disconnected.swap(true, Ordering::SeqCst) {
                log::warn!("MQTT connection lost! Automatic reconnect is active in the background...");
            }
            return Ok(());
        }

        if self.was_disconnected.swap(false, Ordering::SeqCst) {
            log::info!("MQTT connection re-established successfully.");
        }

        macro_rules! pub_msg {
            ($label:expr, $val:expr) => {
                self.client
                    .publish(paho_mqtt::Message::new(
                        format!("{}/{}", self.topic, $label),
                        $val.to_string(),
                        self.qos,
                    ))
                    .with_context(|| "Cannot publish MQTT message")?;
            };
        }

        // EVCC1 topics: status, status_code, max_current, power_or_current
        pub_msg!("status", values.status.0.as_char());
        pub_msg!("status_description", values.status.0.description());
        pub_msg!("status_code", values.status.0.raw_code());
        pub_msg!("max_current", values.max_current.0);
        pub_msg!("enabled", values.max_current.is_enabled());
        pub_msg!("power_or_current", values.power_or_current.0);

        if !no_json {
            let payload = serde_json::to_string(values)?;
            let msg = paho_mqtt::Message::new(
                format!("{}/JSON", self.topic),
                payload,
                self.qos,
            );
            self.client
                .publish(msg)
                .with_context(|| "Cannot publish MQTT message")?;
        }

        Ok(())
    }

    pub fn disconnect(&self) {
        if self.client.is_connected() {
            log::info!("Disconnecting from MQTT broker...");
            if let Err(e) = self.client.disconnect(None) {
                log::warn!("Failed to disconnect cleanly from MQTT broker: {e}");
            } else {
                log::info!("Disconnected cleanly from MQTT broker.");
            }
        }
    }
}
