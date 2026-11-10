use anyhow::Result;
use log::{error, info, warn};
use abl_evcc_lib::tokio_sync_safe_client::SafeClient;
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use crate::{commandline, mqtt};

pub fn run(
    client: &mut SafeClient,
    output: commandline::DaemonOutput,
    interval: Duration,
    no_json: bool,
) -> Result<()> {
    let mqtt_publisher = match &output {
        commandline::DaemonOutput::Mqtt { mqtt_config } => {
            Some(mqtt::MqttPublisher::new(mqtt_config)?)
        }
        commandline::DaemonOutput::Console => None,
    };

    let shutdown_pair = Arc::new((Mutex::new(false), Condvar::new()));

    if let Err(e) = ctrlc::set_handler({
        let shutdown_pair = shutdown_pair.clone();
        move || {
            info!("Received shutdown signal. Stopping daemon gracefully...");
            let (lock, cvar) = &*shutdown_pair;
            if let Ok(mut shutdown) = lock.lock() {
                *shutdown = true;
                cvar.notify_all();
            }
        }
    }) {
        warn!("Failed to set Ctrl-C handler: {e}");
    }

    let (lock, cvar) = &*shutdown_pair;

    loop {
        {
            let shutdown = lock.lock().unwrap();
            if *shutdown {
                break;
            }
        }

        match client.read_all(&interval) {
            Ok(values) => match &output {
                commandline::DaemonOutput::Console => {
                    if no_json {
                        println!("{values}");
                    } else if let Ok(json) = serde_json::to_string_pretty(&values) {
                        println!("{json}");
                    } else {
                        error!("Failed to serialize values to JSON");
                    }
                }
                commandline::DaemonOutput::Mqtt { .. } => {
                    if let Some(publisher) = &mqtt_publisher {
                        if let Err(e) = publisher.publish_values(&values, no_json) {
                            error!("Failed to publish values to MQTT: {e}");
                        }
                    }
                }
            },
            Err(e) => {
                error!("Error reading values from device: {e}");
            }
        }

        let shutdown = lock.lock().unwrap();
        if *shutdown {
            break;
        }

        let _result = cvar.wait_timeout(shutdown, interval).unwrap();
    }

    if let Some(publisher) = &mqtt_publisher {
        publisher.disconnect();
    }

    info!("Daemon stopped cleanly.");
    Ok(())
}

pub fn run_ascii(
    client: &mut abl_evcc_lib::ascii_client::AsciiClient,
    output: commandline::DaemonOutput,
    interval: Duration,
    no_json: bool,
) -> Result<()> {
    let mqtt_publisher = match &output {
        commandline::DaemonOutput::Mqtt { mqtt_config } => {
            Some(mqtt::MqttPublisher::new(mqtt_config)?)
        }
        commandline::DaemonOutput::Console => None,
    };

    let firmware = client.read_firmware().ok();

    let shutdown_pair = Arc::new((Mutex::new(false), Condvar::new()));

    if let Err(e) = ctrlc::set_handler({
        let shutdown_pair = shutdown_pair.clone();
        move || {
            info!("Received shutdown signal. Stopping daemon gracefully...");
            let (lock, cvar) = &*shutdown_pair;
            if let Ok(mut shutdown) = lock.lock() {
                *shutdown = true;
                cvar.notify_all();
            }
        }
    }) {
        warn!("Failed to set Ctrl-C handler: {e}");
    }

    let (lock, cvar) = &*shutdown_pair;

    loop {
        {
            let shutdown = lock.lock().unwrap();
            if *shutdown {
                break;
            }
        }

        match client.read_measurements() {
            Ok(meas) => match &output {
                commandline::DaemonOutput::Console => {
                    if no_json {
                        println!("{meas}");
                    } else if let Ok(json) = serde_json::to_string_pretty(&meas) {
                        println!("{json}");
                    } else {
                        error!("Failed to serialize measurements to JSON");
                    }
                }
                commandline::DaemonOutput::Mqtt { .. } => {
                    if let Some(publisher) = &mqtt_publisher {
                        if let Err(e) = publisher.publish_evcc2_measurements(&meas, firmware.as_ref(), no_json) {
                            error!("Failed to publish measurements to MQTT: {e}");
                        }
                    }
                }
            },
            Err(e) => {
                error!("Error reading measurements from EVCC2/3 device: {e}");
            }
        }

        let shutdown = lock.lock().unwrap();
        if *shutdown {
            break;
        }

        let _result = cvar.wait_timeout(shutdown, interval).unwrap();
    }

    if let Some(publisher) = &mqtt_publisher {
        publisher.disconnect();
    }

    info!("Modbus ASCII Daemon stopped cleanly.");
    Ok(())
}
