# ABL EVCC1 Wallbox Modbus Library and Tool

This repository contains a Rust library and a command-line tool for interacting with ABL EVCC1 series charge controllers (e.g. used in ABL eMH1 wallboxes) via the Modbus protocol (ASCII / RTU / TCP).

## Table of Contents
- [EVCC1 Modbus Register Mapping](#evcc1-modbus-register-mapping)
- [Hardware Requirements](#hardware-requirements)
- [Technical Documentation](#technical-documentation)
- [Technical Specifications](#technical-specifications)
- [Installation & Compilation](#installation--compilation)
- [Command-Line Usage](#command-line-usage)
- [MQTT Configuration](#mqtt-configuration-mqttyaml--environment-variables)
- [Library Usage](#library-usage)
- [Cargo Features](#cargo-features)
- [License](#license)

## EVCC1 Modbus Register Mapping
| Register (Dezimal) | Typ / Richtung | Funktion / Beschreibung bei EVCC1 |
|:---:|:---:|:---|
| **256** (`0x0100`) | Read (Holding) | **Ladestatus (Fahrzeug-Status):** Liefert numerische ASCII-Werte zurück (z. B. 65 = 'A' Standby/Bereit, 66 = 'B' Fahrzeug verbunden, 67 = 'C' Ladevorgang aktiv, 68 = 'D' Belüftung, 69 = 'E', 70 = 'F' Fehler). |
| **257** (`0x0101`) | Read / Write | **Maximaler Ladestrom (Vorgabe / Status):** Dient gleichzeitig als Statusabfrage für die Freigabe und zum Einstellen der Stromstärke (z. B. Wert 16 für 16A, 0 für Ladestopp / Freigabe entzogen, Bereich 6..80A bzw. 0). |
| **258** (`0x0102`) | Read (Holding) | **Aktuelle Ladeleistung / Stromstärke:** Rohwert für Leistung oder Phasenströme je nach Firmware der eMH1. |

## Hardware Requirements
To use this tool, you need:
- An **ABL Wallbox (e.g., eMH1)** or any EVSE with an **EVCC1 charge controller**.
- A **USB-to-RS485 converter** (for RTU/ASCII mode) or a **Modbus TCP gateway**.

## Technical Documentation
For more detailed information, please refer to the documentation in the [`docs/`](./docs/) directory:
- [`docs/EVCC1_Modbus_Register.md`](./docs/EVCC1_Modbus_Register.md) – Nachweise und detaillierte Registerspezifikation.
- [`docs/EVCC.pdf`](./docs/EVCC.pdf) – Offizielle ABL SURSUM EVCC Montage- und Bedienungsanleitung (Dok. Nr. 03 0009 01 D).
- [`docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf`](./docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf) – Offizielle ABL EVCC2/3 Schnittstellenspezifikation.

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
### Read All Values via RTU (RS485)
```sh
abl-evcc rtu --device /dev/ttyUSB0 --baud-rate 38400 --address 1 read-all
```
### Read Status (Register 256)
```sh
abl-evcc rtu --address 1 read-status
```
### Read & Set Charging Current (Register 257)
```sh
# Read current limit:
abl-evcc rtu --address 1 read-current

# Set charging current to 16A:
abl-evcc rtu --address 1 set-current 16

# Stop charging (0A):
abl-evcc rtu --address 1 set-current 0
```
### Read Power or Current (Register 258)
```sh
abl-evcc rtu --address 1 read-power
```
### Read Values via TCP Gateway
```sh
abl-evcc tcp 192.168.1.100:502 read-all
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

  Published topics:
  - `<topic>/status`: ASCII letter (`A`, `B`, `C`, etc.)
  - `<topic>/status_code`: ASCII integer (65, 66, 67, etc.)
  - `<topic>/status_description`: Human-readable status description
  - `<topic>/max_current`: Configured charging current in Amperes
  - `<topic>/enabled`: Boolean indicating if charging is allowed (`true` if current >= 6A)
  - `<topic>/power_or_current`: Measurement value from register 258
  - `<topic>/JSON`: Full JSON object with all values

## Library Usage
The `abl_evcc_lib` crate provides high-level and low-level interfaces for EVCC1 controllers:

```rust
use abl_evcc_lib::tokio_sync_safe_client::SafeClient;
use std::time::Duration;
use tokio_modbus::client::sync::tcp;
use tokio_modbus::Slave;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr = "192.168.1.100:502".parse()?;
    let ctx = tcp::connect_slave(socket_addr, Slave(1))?;
    let client = SafeClient::new(ctx);

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
