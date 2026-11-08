# ABL EVCC Wallbox Modbus Library and Tool

This repository contains a Rust library and a command-line tool for interacting with ABL EVCC2/3 series charge controllers (e.g. used in ABL eMH1/eMH2/eMH3 wallboxes) via the Modbus ASCII/RTU protocol.

## Table of Contents
- [Hardware Requirements](#hardware-requirements)
- [Technical Documentation](#technical-documentation)
- [Technical Specifications](#technical-specifications)
- [Installation & Compilation](#installation--compilation)
- [Command-Line Usage](#command-line-usage)
- [MQTT Configuration](#mqtt-configuration-mqttyaml--environment-variables)
- [Library Usage](#library-usage)
- [Cargo Features](#cargo-features)
- [License](#license)

## Hardware Requirements
To use this tool, you need:
- An **ABL Wallbox (e.g., eMH1, eMH2, eMH3)** or any EVSE with an **EVCC2/3 charge controller**.
- A **USB-to-RS485 converter** (for RTU mode) or a **Modbus TCP gateway**.

## Technical Documentation
For more detailed information, please refer to the official specification in the [`docs/`](./docs/) directory:
- [`Schnittstellenbeschreibung_Modbus-ASCII.pdf`](./docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf)

## Technical Specifications
| Feature | Details |
|---|---|
| **Default Baud Rate** | 38400 Bd |
| **Data Format** | 8E1 (8 data bits, Even parity, 1 stop bit) |
| **Supported Functions**| 0x03 (Read holding registers), 0x10 (Write multiple registers) |
| **Device IDs** | 0x01..0x10 (0x00 for Broadcast) |
| **Checksum** | Longitudinal Redundancy Check (LRC) / Modbus RTU CRC |

## Installation & Compilation

### Prerequisites
Ensure you have the following dependencies installed:
- **Rust and Cargo**: Install via [rustup](https://rustup.rs/)
- **Git**: To clone the repository
- A C compiler and linker

### Building from Source
1. **Clone the repository**:
   ```sh
   git clone https://github.com/acpiccolo/ABL-EVCC-Wallbox.git
   cd ABL-EVCC-Wallbox
   ```
2. **Compile the project**:
   ```sh
   cargo build --release
   ```
   The compiled binary will be available at:
   ```sh
   target/release/abl-evcc
   ```
3. **(Optional) Install the binary system-wide**:
   ```sh
   cargo install --path .
   ```

## Command-Line Usage
### View Available Commands
```sh
abl-evcc --help
```
### Read Values via RTU (RS485)
```sh
abl-evcc rtu --device /dev/ttyUSB0 --baud-rate 38400 --address 1 read-all
```
### Read Values via TCP Gateway
```sh
abl-evcc tcp 192.168.1.100:502 read-all
```
### Set Charging Current
```sh
abl-evcc rtu --address 1 set-current 16.0
```

### Daemon Mode with MQTT
You can run the tool as a daemon that publishes data to an MQTT broker.

- **Log to Console:**
  ```sh
  abl-evcc rtu --address 1 daemon console
  ```

- **Publish to MQTT:**
  ```sh
  abl-evcc rtu --address 1 daemon mqtt --mqtt-config /path/to/mqtt.yaml
  ```
  If `--mqtt-config` is omitted, `mqtt.yaml` is loaded from the current directory.

### MQTT Configuration (mqtt.yaml & Environment Variables)

All MQTT connection parameters are configured through a YAML file. This approaches centralizes settings like the broker URL, credentials, topic, QoS, and client ID.

**Path Specification:**
- Use the `--mqtt-config <PATH>` option to specify the path to your MQTT configuration file.
- If omitted, the application will attempt to load `mqtt.yaml` from the current working directory.

**Mandatory Fields:**
- The `uri` field within the MQTT configuration YAML file is **mandatory**.

#### Environment Variables Override

You can override or specify configuration values via environment variables:
* `MQTT_URI`: Overrides `uri`
* `MQTT_USERNAME`: Overrides `username`
* `MQTT_PASSWORD`: Overrides `password`
* `MQTT_TOPIC`: Overrides `topic`
* `MQTT_CLIENT_ID`: Overrides `client_id`

## Library Usage
The `abl_evcc_lib` crate provides two main ways to interact with EVCC charge controllers:

1. **High-Level, Safe Clients**: Stateful, thread-safe clients (`SafeClient` in `tokio_sync_safe_client` and `tokio_async_safe_client`).
2. **Low-Level Stateless Functions**: Direct mapping in `tokio_sync` and `tokio_async`.

### Quick Start: Synchronous Client

```rust
use abl_evcc_lib::tokio_sync_safe_client::SafeClient;
use std::time::Duration;
use tokio_modbus::client::sync::tcp;
use tokio_modbus::Slave;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr = "192.168.1.100:502".parse()?;
    let ctx = tcp::connect_slave(socket_addr, Slave(1))?;
    let mut client = SafeClient::new(ctx);

    let values = client.read_all(&Duration::from_millis(50))?;
    println!("Values: {:#?}", values);

    Ok(())
}
```

## Cargo Features
- **`default`**: Enables `bin-dependencies` for the CLI tool.
- **`tokio-rtu-sync`**: Synchronous RTU client.
- **`tokio-tcp-sync`**: Synchronous TCP client.
- **`tokio-rtu`**: Asynchronous RTU client.
- **`tokio-tcp`**: Asynchronous TCP client.
- **`safe-client-sync`**: Thread-safe synchronous wrapper.
- **`safe-client-async`**: Thread-safe asynchronous wrapper.
- **`serde`**: Serialization/deserialization support for protocol structs.

## License
Licensed under either of Apache License, Version 2.0 or MIT license at your option.
