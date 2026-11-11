# ABL eMH1 Wallbox Tool (Modbus ASCII)

This repository contains a Rust library and command-line tool for interacting with **ABL eMH1** (and eMH2 / EVCC2/3) wallboxes over **Modbus ASCII**.

## Table of Contents
- [Overview](#overview)
- [Modbus ASCII Registers](#modbus-ascii-registers)
- [Ampere vs. Watt Definition](#ampere-vs-watt-definition)
- [Connection Methods (`serial` vs `tcp`)](#connection-methods-serial-vs-tcp)
- [Available CLI Commands](#available-cli-commands)
- [Daemon Mode & MQTT (Bidirectional Control)](#daemon-mode--mqtt-bidirectional-control)
- [Library Usage](#library-usage)
- [Cargo Features](#cargo-features)
- [Technical Documentation](#technical-documentation)
- [License](#license)

---

## Overview
ABL eMH1 wallboxes communicate via **Modbus ASCII** (default 38400 Baud, 8E1).
Requests start with `:` (`0x3A`), responses start with `>` (`0x3E`), and frames are terminated with `\r\n` (`0x0D 0x0A`) with a Longitudinal Redundancy Check (LRC) checksum.

This tool provides two direct connection modes:
- **`tcp`**: Connects directly via TCP to transparent serial servers (such as **Elfin EW11**, USR-TCP232, Protoss-PE11, etc.) without requiring `socat`.
- **`serial`**: Connects via local serial devices or virtual serial ports (default: `/dev/ttyUSB0`, or Windows COM ports) with configurable baud rate, parity, stop bits, address, and automatic serial delay timing verification.

---

## Ampere vs. Watt Definition

- **Ampere (`set-current`) ist pro Phase:**
  Die Wallbox signalisiert über das Control-Pilot-PWM-Signal die maximale Stromstärke pro Phase (6A bis 80A, oder 0A zum Stoppen).
- **Watt (`set-power`) gilt für 3 Phasen:**
  Die Wattangabe wird für ein 3-phasiges Netz ($3 \times 230\,\text{V} = 690\,\text{V}$) automatisch in die entsprechende Stromstärke pro Phase umgerechnet:
  $$\text{Ampere pro Phase} = \text{round}\left(\frac{\text{Leistung in Watt}}{3 \times 230\,\text{V}}\right)$$
  - Beispiel: `11000 W` $\rightarrow$ `16 A` pro Phase
  - Beispiel: `22000 W` $\rightarrow$ `32 A` pro Phase
  - Beispiel: `0 W` $\rightarrow$ `0 A` (Ladestopp)

---

## Modbus ASCII Registers

The tool implements all register definitions from the official ABL EVCC2/3 specification:

| Register | Name / Function | Type | Description |
|:---:|:---|:---:|:---|
| **`0x0001`** | Read Device ID & Firmware | Read (2 words) | Hardware/PCBA revision, firmware version, and hardware flags (internal current sensor, RDC-MD, contactor monitor). |
| **`0x0003`** | Read Modbus Settings | Read (1 word) | Current baud rate (9600..57600 Bd), parity (None/Odd/Even), and stop bits (1 or 2). |
| **`0x0004`** | Read State Machine Status | Read (1 word) | Detailed state machine position: `0xA1` (Waiting for EV), `0xB1` (EV asking for charge), `0xB2` (EV permitted to charge), `0xC2` (Charging), error codes `0xF1`–`0xFB`. |
| **`0x0005`** | Modify State / Reset | Write (1 word) | `0xE0E0` (jump to E0 outlet disabled), `0xA1A1` (jump to A1 waiting for EV), `0x5A5A` (restart controller). Used for vehicle wake-up and reset. |
| **`0x0006`** | Read System Flags | Read (2 words) | State machine pointer, contactor feedback, upstream communication status, load imbalance, and overtemperature indicators. |
| **`0x0014`** | Set Maximum Current / Duty Cycle | Write (1 word) | Duty cycle in 0.1% steps (`Amperes / 0.06`). `1000` (`0x03E8`) = disabled / charging prohibited. |
| **`0x002E`** | Full Status & Phase Currents | Read (5 words) | Status byte, EV connection flag, configured current, and 3-phase currents in 0.1A resolution (`I_L1`, `I_L2`, `I_L3`). |

---

## Connection Methods (`serial` vs `tcp`)

### 1. `tcp` Mode (Direct Network Connection via Serial Server, e.g. Elfin EW11)
Connect directly without needing `socat`:
```bash
abl-evcc tcp 192.168.3.128:8899 read-status
```
Options:
- `-s, --slave <SLAVE>`: Modbus slave ID (default: `1`)

### 2. `serial` Mode (Local Serial Port)
Connect over RS485 USB converter (default: `/dev/ttyUSB0`):
```bash
abl-evcc serial --device /dev/ttyUSB0 read-status
```
Configurable options:
- `-d, --device <DEVICE>`: Serial device path (default: `/dev/ttyUSB0`)
- `--baud-rate <BAUD>`: Baud rate (default: `38400`)
- `--parity-and-stop-bit <PARITY>`: Parity and stop bits (`ep1b` for 8E1 [default], `np1b` for 8N1, `op1b` for 8O1, `np2b` for 8N2)
- `--address <ADDRESS>`: Modbus slave address 1..16 (default: `1`)

---

## Available CLI Commands

Both `tcp` and `serial` support all wallbox commands:

### Read Wallbox Charging Status (`read-status`)
```bash
abl-evcc tcp 192.168.3.128:8899 read-status
```

### Read All Measurements & Diagnostics (`read-all`)
Reads firmware info, system flags, and full measurements:
```bash
abl-evcc tcp 192.168.3.128:8899 read-all
```

### Read 3-Phase Currents & Active Power (`read-currents`)
```bash
abl-evcc tcp 192.168.3.128:8899 read-currents
```

### Read Configured Charging Current Limit (`read-current`)
```bash
abl-evcc tcp 192.168.3.128:8899 read-current
```

### Set Charging Current in Amperes per phase (`set-current`)
```bash
# Set limit to 16 Amperes per phase:
abl-evcc tcp 192.168.3.128:8899 set-current 16

# Stop / halt charging (0 Amperes, prompts for confirmation unless -f / --force is given):
abl-evcc tcp 192.168.3.128:8899 set-current 0 --force
```

### Set Charging Power in Watts for 3 phases (`set-power`)
```bash
# Set limit to 11000 Watts (3-phase, converts to 16A per phase):
abl-evcc tcp 192.168.3.128:8899 set-power 11000

# Stop / halt charging (0 Watts, prompts for confirmation unless -f / --force is given):
abl-evcc tcp 192.168.3.128:8899 set-power 0 --force
```

### Enable / Disable Charging (`enable`, `disable`)
```bash
# Enable at 16A per phase:
abl-evcc tcp 192.168.3.128:8899 enable 16

# Enable specifying 3-phase power:
abl-evcc tcp 192.168.3.128:8899 enable --power 11000

# Disable charging (prompts for confirmation unless -f / --force is given):
abl-evcc tcp 192.168.3.128:8899 disable --force
```

### Read Firmware & Hardware Configuration (`read-firmware`)
```bash
abl-evcc tcp 192.168.3.128:8899 read-firmware
```

### Read Wallbox System Flags & State Machine Pointer (`read-system-flags`)
```bash
abl-evcc tcp 192.168.3.128:8899 read-system-flags
```

### Read Modbus Communication Settings (`read-modbus-settings`)
```bash
abl-evcc tcp 192.168.3.128:8899 read-modbus-settings
```

### Wake Up Vehicle (`wake-up`)
Cycles Control Pilot state (E0 -> wait 3s -> A1) to wake up sleeping EV onboard chargers:
```bash
abl-evcc tcp 192.168.3.128:8899 wake-up
```

### Reset Controller (`reset`)
Sends command `0x5A5A` to restart the EVCC2 controller (prompts for confirmation unless `-f / --force` is given):
```bash
abl-evcc tcp 192.168.3.128:8899 reset --force
```

---

## Daemon Mode & MQTT (Bidirectional Control)

Continuously polls wallbox data and logs to console or publishes to an MQTT broker:

- **Log to Console:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon console
  ```

- **Publish to MQTT Broker:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon mqtt --mqtt-config mqtt.yaml
  ```

### Published MQTT Topics:
- `<topic>/status`: Character (`A`, `B`, `C`, `D`, `E`, `F`)
- `<topic>/status_code`: Hex code (`0x00A1`, `0x00B1`, `0x00C2`, etc.)
- `<topic>/status_description`: Human-readable status description
- `<topic>/ev_connected`: Boolean (`true`/`false`)
- `<topic>/configured_current`: Configured charging current in Amperes per phase
- `<topic>/enabled`: Boolean indicating if charging is active (`true` if current >= 6A)
- `<topic>/current_l1`: Phase 1 current in Amperes
- `<topic>/current_l2`: Phase 2 current in Amperes
- `<topic>/current_l3`: Phase 3 current in Amperes
- `<topic>/power`: Total calculated active charging power in Watts
- `<topic>/JSON`: Full JSON object with all measurement values

### Bidirectional Control via MQTT:
When `allow_control: true` is set in `mqtt.yaml` (default: `false`), the daemon subscribes to `<topic>/set/#` and allows remote control:
- `<topic>/set/current`: Sets maximum current in Amperes per phase (e.g. `16` or `0` to stop). Capped by `max_current`.
- `<topic>/set/power`: Sets 3-phase power in Watts (e.g. `11000` or `0` to stop). Capped by `max_current`.
- `<topic>/set/enable`: Boolean (`true` / `on` to enable at `max_current`, `false` / `off` to disable).
- `<topic>/set/wake_up`: Set to `true` / `1` to trigger vehicle wake-up sequence.
- `<topic>/set/reset`: Set to `true` / `1` to restart the EVCC2 charge controller.

#### MQTT `max_current` Safety Limit:
In `mqtt.yaml`, `max_current` (default: `1`) defines the **maximum charging current in Amperes per phase** that can be requested via MQTT. Any incoming current or power command exceeding this limit is automatically capped, and when enabling charging via MQTT, `max_current` is used as the target current.

---

## Library Usage

The `abl_evcc_lib` crate provides two main ways to interact with the ABL eMH1 energy/charge controllers:

1. **High-Level, Safe Clients**: Stateful, thread-safe clients that are easy to share and use in concurrent applications. This is the recommended approach for most users. See `tokio_sync_safe_client::SafeClient` (blocking) and `tokio_async_safe_client::SafeClient` (`async`).

2. **Low-Level, Stateless Functions**: A set of stateless functions that directly map to the device's Modbus commands. This API offers maximum flexibility but requires manual management of the Modbus context. See the `tokio_sync` and `tokio_async` modules.

### Quick Start: Synchronous Safe Client

```rust
use abl_evcc_lib::{
    tokio_sync::AsciiClient,
    tokio_sync_safe_client::SafeClient,
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect directly to a transparent TCP serial server (e.g. Elfin EW11)
    let ascii_client = AsciiClient::connect_tcp("192.168.3.128:8899", 1, Duration::from_secs(3), Duration::from_millis(100))?;
    let mut client = SafeClient::new(ascii_client);

    // Read full measurements (status, 3-phase currents, active power, current limit)
    let measurements = client.read_measurements()?;
    println!("Successfully read measurements:\n{}", measurements);

    // Set maximum charging current to 16A per phase
    client.set_current(16)?;

    Ok(())
}
```

### Quick Start: Asynchronous Safe Client (TCP and Serial)

```rust
use abl_evcc_lib::tokio_async_safe_client::SafeClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Via direct TCP:
    let mut tcp_client = SafeClient::connect_tcp("192.168.3.128:8899", 1, Duration::from_millis(100)).await?;
    let measurements = tcp_client.read_measurements().await?;
    println!("Async TCP read status: {:?}", measurements.status);

    // Or via asynchronous serial port (/dev/ttyUSB0):
    // let mut serial_client = SafeClient::connect_serial(
    //     "/dev/ttyUSB0", 38400, tokio_serial::Parity::Even, tokio_serial::StopBits::One, 1, Duration::from_millis(100),
    // )?;

    Ok(())
}
```

---

## Cargo Features

This crate uses a feature-based system to minimize dependencies:

- **`default`**: Enables `bin-dependencies` for compiling the `abl-evcc` CLI tool.
- **`safe-client-sync`**: Thread-safe synchronous wrapper [`tokio_sync_safe_client::SafeClient`].
- **`safe-client-async`**: Thread-safe asynchronous wrapper [`tokio_async_safe_client::SafeClient`].
- **`tokio-sync`**: Stateless synchronous functions [`tokio_sync::Evcc2`].
- **`tokio-async`**: Stateless asynchronous functions [`tokio_async::Evcc2Async`].
- **`serde`**: Implements serialization/deserialization for protocol data types.

---

## Technical Documentation
- [`docs/ABL_eMH1_Command_and_Value_Reference_DE.md`](./docs/ABL_eMH1_Command_and_Value_Reference_DE.md) – Ausführliche Befehls-, Werte- und Register-Referenz (Deutsch) mit Glossar (CP, CS, PWM, Duty Cycle, RDC-MD, etc.) und detaillierten Felddefinitionen.
- [`docs/ABL_eMH1_Command_and_Value_Reference_EN.md`](./docs/ABL_eMH1_Command_and_Value_Reference_EN.md) – Comprehensive Command, Value, and Register Reference (English) with glossary and field specifications.
- [`docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf`](./docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf) – Official ABL EVCC2/3 Modbus ASCII specification.

## License
Licensed under either Apache License, Version 2.0 or MIT license at your option.
