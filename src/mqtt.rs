use anyhow::{Context, Result};
use paho_mqtt::{Client, ConnectOptionsBuilder, CreateOptionsBuilder};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
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
        let config_file = std::fs::File::open(config_file_path)
            .with_context(|| format!("Cannot open MQTT config file {:?}", config_file_path))?;
        let config: Self = serde_yaml::from_reader(&config_file)
            .with_context(|| format!("Cannot read MQTT config from file: {:?}", config_file_path))?;
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

        client
            .connect(conn_opts)
            .with_context(|| "Failed to connect to MQTT broker")?;
        Ok(client)
    }
}

pub fn run_mqtt_daemon(
    client: &mut abl_evcc_lib::tokio_sync_safe_client::SafeClient,
    delay: &Duration,
    poll_interval: &Duration,
    config_file: &str,
    no_json: bool,
) -> Result<()> {
    let config = MqttConfig::load(config_file)?;
    let mqtt_client = config.create_client()?;

    loop {
        let current = client
            .read_current_full()
            .with_context(|| "Cannot read current values")?;

        macro_rules! pub_msg {
            ($label:expr, $val:expr) => {
                mqtt_client.publish(paho_mqtt::Message::new(
                    format!("{}/{}", config.topic, $label),
                    $val.to_string(),
                    config.qos,
                ))
                .with_context(|| "Cannot publish MQTT message")?;
            };
        }

        pub_msg!("state", format!("{:#04X}", current.evse_state));
        pub_msg!("ev_connected", current.ucp_le_10v);
        pub_msg!("duty_cycle_percent", (current.duty_cycle_ic_x10 as f32) / 10.0);
        pub_msg!("l1_current", (current.ict1_x10_decia as f32) / 10.0);
        pub_msg!("l2_current", (current.ict2_x10_decia as f32) / 10.0);
        pub_msg!("l3_current", (current.ict3_x10_decia as f32) / 10.0);

        if !no_json {
            let payload = serde_json::to_string(&current)?;
            let msg = paho_mqtt::Message::new(
                format!("{}/JSON", config.topic),
                payload,
                config.qos,
            );
            mqtt_client
                .publish(msg)
                .with_context(|| "Cannot publish MQTT message")?;
        }

        std::thread::sleep(*delay.max(poll_interval));
    }
}
