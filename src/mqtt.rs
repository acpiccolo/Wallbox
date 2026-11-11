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
    /// Allow controlling the wallbox via MQTT subscriptions (default: false)
    #[serde(default = "MqttConfig::default_allow_control")]
    pub allow_control: bool,
    /// Maximum charging current allowed via MQTT in Amperes per phase (default: 1A)
    #[serde(default = "MqttConfig::default_max_current")]
    pub max_current: u16,
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
            .field("password", &self.password.as_ref().map(|_| "********"))
            .field("topic", &self.topic)
            .field("qos", &self.qos)
            .field("client_id", &self.client_id)
            .field("operation_timeout", &self.operation_timeout)
            .field("keep_alive_interval", &self.keep_alive_interval)
            .field(
                "auto_reconnect_interval_min",
                &self.auto_reconnect_interval_min,
            )
            .field(
                "auto_reconnect_interval_max",
                &self.auto_reconnect_interval_max,
            )
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

    fn default_allow_control() -> bool {
        false
    }

    fn default_max_current() -> u16 {
        1
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
        let mut config: Self = yaml_serde::from_reader(&config_file).with_context(|| {
            format!("Cannot read MQTT config from file: {:?}", config_file_path)
        })?;

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
        if let Ok(env_ctrl) = std::env::var("MQTT_ALLOW_CONTROL") {
            if let Ok(val) = env_ctrl.parse::<bool>() {
                config.allow_control = val;
            }
        }
        if let Ok(env_max) = std::env::var("MQTT_MAX_CURRENT") {
            if let Ok(val) = env_max.parse::<u16>() {
                config.max_current = val;
            }
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
    allow_control: bool,
    max_current: u16,
    consumer: Option<paho_mqtt::SyncReceiver<Option<paho_mqtt::Message>>>,
    was_disconnected: AtomicBool,
}

impl MqttPublisher {
    pub fn new(config_file: &str) -> Result<Self> {
        let config = MqttConfig::load(config_file)?;
        let client = config.create_client()?;

        let consumer = if config.allow_control {
            log::info!("MQTT remote control is ENABLED. Setting up consumer...");
            let rx = client.start_consuming();
            let sub_topic = format!("{}/set/#", config.topic);
            log::info!("Subscribing to command topic: {sub_topic}");
            client
                .subscribe(&sub_topic, config.qos)
                .with_context(|| format!("Failed to subscribe to {sub_topic}"))?;
            Some(rx)
        } else {
            log::info!("MQTT remote control is disabled (read-only mode).");
            None
        };

        Ok(Self {
            client,
            topic: config.topic,
            qos: config.qos,
            allow_control: config.allow_control,
            max_current: config.max_current,
            consumer,
            was_disconnected: AtomicBool::new(false),
        })
    }

    pub fn allow_control(&self) -> bool {
        self.allow_control
    }

    /// Check for incoming control messages without blocking.
    pub fn check_incoming_messages(
        &self,
        wallbox: &mut abl_evcc_lib::tokio_sync::AsciiClient,
    ) -> Result<()> {
        let Some(consumer) = &self.consumer else {
            return Ok(());
        };

        while let Ok(Some(msg)) = consumer.try_recv() {
            let topic = msg.topic();
            let payload = msg.payload_str();
            log::info!("Received MQTT command on topic '{}': '{}'", topic, payload);

            let prefix = format!("{}/set/", self.topic);
            if let Some(cmd) = topic.strip_prefix(&prefix) {
                match cmd {
                    "current" => {
                        if let Ok(mut amps) = payload.trim().parse::<u16>() {
                            if amps > 0 && amps > self.max_current {
                                log::warn!(
                                    "Requested current {}A per phase exceeds configured MQTT max_current limit ({}A per phase). Capping to {}A.",
                                    amps, self.max_current, self.max_current
                                );
                                amps = self.max_current;
                            }
                            if amps == 0 || (6..=80).contains(&amps) {
                                log::info!("Applying MQTT current setting: {}A per phase", amps);
                                if let Err(e) = wallbox.set_current(amps) {
                                    log::error!("Failed to set current via MQTT: {e}");
                                }
                            } else {
                                log::warn!("Current value {}A per phase is out of EVCC2 range (0 or 6..=80A)", amps);
                            }
                        }
                    }
                    "power" => {
                        if let Ok(watts) = payload.trim().parse::<u32>() {
                            let mut amps = abl_evcc_lib::protocol::power_watts_to_amperes(watts);
                            if amps > 0 && amps > self.max_current {
                                log::warn!(
                                    "Requested power {}W converts to {}A per phase, which exceeds MQTT max_current limit ({}A per phase). Capping to {}A.",
                                    watts, amps, self.max_current, self.max_current
                                );
                                amps = self.max_current;
                            }
                            if amps == 0 || (6..=80).contains(&amps) {
                                log::info!(
                                    "Applying MQTT power setting: {}W -> {}A per phase",
                                    watts,
                                    amps
                                );
                                if let Err(e) = wallbox.set_current(amps) {
                                    log::error!("Failed to set power via MQTT: {e}");
                                }
                            } else {
                                log::warn!("Power value {}W converted to {}A per phase, which is out of range (0 or 6..=80A)", watts, amps);
                            }
                        }
                    }
                    "enable" => {
                        let text = payload.trim().to_lowercase();
                        if text == "true" || text == "1" || text == "on" {
                            let enable_amps = self.max_current;
                            log::info!("Enabling charging via MQTT at configured max_current: {}A per phase", enable_amps);
                            if enable_amps == 0 || (6..=80).contains(&enable_amps) {
                                if let Err(e) = wallbox.enable(Some(enable_amps)) {
                                    log::error!("Failed to enable charging via MQTT: {e}");
                                }
                            } else {
                                log::warn!("Configured max_current ({}A) is below minimum 6A required to enable charging", enable_amps);
                            }
                        } else if text == "false" || text == "0" || text == "off" {
                            log::info!("Disabling charging via MQTT");
                            if let Err(e) = wallbox.disable() {
                                log::error!("Failed to disable charging via MQTT: {e}");
                            }
                        }
                    }
                    "wake_up" => {
                        let text = payload.trim().to_lowercase();
                        if text == "true" || text == "1" {
                            log::info!("Triggering wake-up sequence via MQTT");
                            if let Err(e) = wallbox.wake_up() {
                                log::error!("Failed to trigger wake-up via MQTT: {e}");
                            }
                        }
                    }
                    "reset" => {
                        let text = payload.trim().to_lowercase();
                        if text == "true" || text == "1" {
                            log::info!("Triggering controller reset via MQTT");
                            if let Err(e) = wallbox.reset() {
                                log::error!("Failed to trigger reset via MQTT: {e}");
                            }
                        }
                    }
                    other => {
                        log::warn!("Unknown MQTT command topic: {}", other);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn publish_measurements(
        &self,
        meas: &abl_evcc_lib::protocol::Evcc2Measurements,
        firmware: Option<&abl_evcc_lib::protocol::Evcc2Firmware>,
        no_json: bool,
    ) -> Result<()> {
        if !self.client.is_connected() {
            if !self.was_disconnected.swap(true, Ordering::SeqCst) {
                log::warn!(
                    "MQTT connection lost! Automatic reconnect is active in the background..."
                );
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

        pub_msg!("status", meas.status.as_char());
        pub_msg!("status_code", format!("{:#04X}", meas.status.raw_code()));
        pub_msg!("status_description", meas.status.description());
        pub_msg!("ev_connected", meas.ev_connected);
        pub_msg!("configured_current", meas.configured_current_amperes);
        pub_msg!("enabled", meas.configured_current_amperes >= 6.0);
        pub_msg!("current_l1", meas.current_l1_amperes);
        pub_msg!("current_l2", meas.current_l2_amperes);
        pub_msg!("current_l3", meas.current_l3_amperes);
        pub_msg!("power", meas.total_power_watts);

        if let Some(fw) = firmware {
            pub_msg!("firmware_revision", fw.firmware_revision());
            pub_msg!("pcba_version", fw.pcba_version);
        }

        if !no_json {
            let payload = serde_json::to_string(meas)?;
            let msg = paho_mqtt::Message::new(format!("{}/JSON", self.topic), payload, self.qos);
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
