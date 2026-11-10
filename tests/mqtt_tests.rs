use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
struct TestMqttConfig {
    pub uri: String,
    #[serde(default = "default_topic")]
    pub topic: String,
    #[serde(default = "default_allow_control")]
    pub allow_control: bool,
    #[serde(default = "default_max_current")]
    pub max_current: u16,
}

fn default_topic() -> String {
    "abl-evcc".into()
}

fn default_allow_control() -> bool {
    false
}

fn default_max_current() -> u16 {
    1
}

#[cfg(test)]
mod mqtt_config_tests {
    use super::*;

    #[test]
    fn test_mqtt_config_defaults() {
        let yaml = r#"
uri: "tcp://localhost:1883"
"#;
        let config: TestMqttConfig = yaml_serde::from_str(yaml).unwrap();
        assert_eq!(config.uri, "tcp://localhost:1883");
        assert_eq!(config.topic, "abl-evcc");
        assert!(!config.allow_control);
        assert_eq!(config.max_current, 1);
    }

    #[test]
    fn test_mqtt_config_custom_values() {
        let yaml = r#"
uri: "tcp://192.168.1.50:1883"
topic: "custom/wallbox"
allow_control: true
max_current: 16
"#;
        let config: TestMqttConfig = yaml_serde::from_str(yaml).unwrap();
        assert_eq!(config.uri, "tcp://192.168.1.50:1883");
        assert_eq!(config.topic, "custom/wallbox");
        assert!(config.allow_control);
        assert_eq!(config.max_current, 16);
    }
}
