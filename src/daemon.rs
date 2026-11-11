use abl_evcc_lib::tokio_sync::AsciiClient;
use anyhow::Result;
use log::{error, info, warn};
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use crate::{commandline, mqtt};

pub fn run(
    client: &mut AsciiClient,
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

        // If MQTT control is active, handle any incoming commands before reading measurements
        if let Some(publisher) = &mqtt_publisher {
            if publisher.allow_control() {
                if let Err(e) = publisher.check_incoming_messages(client) {
                    error!("Error handling incoming MQTT messages: {e}");
                }
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
                        if let Err(e) =
                            publisher.publish_measurements(&meas, firmware.as_ref(), no_json)
                        {
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
