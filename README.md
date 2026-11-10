# ABL eMH1 Wallbox Tool (Modbus ASCII)

This repository contains a Rust library and command-line tool for interacting with **ABL eMH1** (and eMH2 / EVCC2/3) wallboxes over **Modbus ASCII**.

## Table of Contents
- [Overview](#overview)
- [Modbus ASCII Protocol & Registers](#modbus-ascii-protocol--registers)
- [Connection Methods (`serial` vs `tcp`)](#connection-methods-serial-vs-tcp)
- [Command-Line Usage](#command-line-usage)
- [Daemon Mode & MQTT](#daemon-mode--mqtt)
- [Technical Documentation](#technical-documentation)
- [License](#license)

---

## Overview
ABL eMH1 wallboxes communicate via **Modbus ASCII** (default 38400 Baud, 8E1).
Requests start with `:` (`0x3A`), responses start with `>` (`0x3E`), and frames are terminated with `\r\n` (`0x0D 0x0A`) with a Longitudinal Redundancy Check (LRC) checksum.

This tool provides two direct connection modes:
- **`tcp`**: Connects directly via TCP to transparent serial servers (such as **Elfin EW11**, USR-TCP232, Protoss-PE11, etc.) without requiring `socat`.
- **`serial`**: Connects via local serial devices or virtual serial ports (e.g. `/dev/ttyUSB0` or Windows COM ports) with configurable baud rate, parity, stop bits, and address.

---

## Modbus ASCII Protocol & Registers

The tool implements the official ABL EVCC2/3 specification:

| Register | Name / Function | Type | Description |
|:---:|:---|:---:|:---|
| **`0x0001`** | Read Device ID & Firmware | Read (2 words) | Hardware/PCBA revision, firmware version, and hardware flags (internal current sensor, RDC-MD, contactor monitor). |
| **`0x0004`** | Read State Machine Status | Read (1 word) | Detailed state machine position: `0xA1` (Waiting for EV), `0xB1` (EV asking for charge), `0xB2` (EV permitted to charge), `0xC2` (Charging), error codes `0xF1`–`0xFB`. |
| **`0x0005`** | Modify State | Write (1 word) | `0xE0E0` (jump to E0 outlet disabled), `0xA1A1` (jump to A1 waiting for EV), `0x5A5A` (reset). Used for vehicle wake-up. |
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

## Command-Line Usage

Both `tcp` and `serial` support identical subcommands:

### Read Wallbox Charging Status
```bash
# Via direct TCP:
abl-evcc tcp 192.168.3.128:8899 read-status

# Via serial port:
abl-evcc serial --device /dev/ttyUSB0 read-status
```
Example JSON output:
```json
"State A (0x00A1): Waiting for EV (Standby)"
```
With `--no-json`:
```
State A (0x00A1): Waiting for EV (Standby)
```

### Read All Measurements (Status, Currents, Power, Firmware)
```bash
abl-evcc tcp 192.168.3.128:8899 read-all
```

### Read 3-Phase Currents & Active Power
```bash
abl-evcc tcp 192.168.3.128:8899 read-currents
```

### Read Configured Charging Current
```bash
abl-evcc tcp 192.168.3.128:8899 read-current
```

### Set Charging Current
```bash
# Set limit to 16 Amperes:
abl-evcc tcp 192.168.3.128:8899 set-current 16

# Stop / halt charging (0 Amperes):
abl-evcc tcp 192.168.3.128:8899 set-current 0
```

### Enable / Disable Charging
```bash
# Enable at 16A:
abl-evcc tcp 192.168.3.128:8899 enable 16

# Disable charging:
abl-evcc tcp 192.168.3.128:8899 disable
```

### Wake Up Vehicle
Cycles Control Pilot state (E0 -> wait 3s -> A1) to wake up sleeping EV onboard chargers:
```bash
abl-evcc tcp 192.168.3.128:8899 wake-up
```

---

## Daemon Mode & MQTT

Both `tcp` and `serial` modes support continuous polling:

- **Log to Console:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon console
  abl-evcc serial --device /dev/ttyUSB0 daemon console
  ```

- **Publish to MQTT Broker:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon mqtt --mqtt-config mqtt.yaml
  ```

  Published MQTT topics:
  - `<topic>/status`: Character (`A`, `B`, `C`, `D`, `E`, `F`)
  - `<topic>/status_code`: Hex code (`0x00A1`, `0x00B1`, `0x00C2`, etc.)
  - `<topic>/status_description`: Human-readable status description
  - `<topic>/ev_connected`: Boolean (`true`/`false`)
  - `<topic>/configured_current`: Configured charging current in Amperes
  - `<topic>/enabled`: Boolean indicating if charging is active (`true` if current >= 6A)
  - `<topic>/current_l1`: Phase 1 current in Amperes
  - `<topic>/current_l2`: Phase 2 current in Amperes
  - `<topic>/current_l3`: Phase 3 current in Amperes
  - `<topic>/power`: Total calculated active charging power in Watts
  - `<topic>/JSON`: Full JSON object with all measurement values

---

## Technical Documentation
- [`docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf`](./docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf) – Official ABL EVCC2/3 specification.

## License
Licensed under either Apache License, Version 2.0 or MIT license at your option.
