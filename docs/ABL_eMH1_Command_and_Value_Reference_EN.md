# ABL eMH1 (EVCC2/3) Command, Value, and Register Reference

This document provides complete, field-level documentation of all commands, configurable values, response payloads, technical abbreviations, and register definitions for **ABL eMH1** (and eMH2 / EVCC2/3) wallboxes.

---

## Table of Contents
1. [Glossary and Technical Abbreviations](#1-glossary-and-technical-abbreviations)
2. [Status Codes (IEC 61851-1 & ABL EVCC2)](#2-status-codes-iec-61851-1--abl-evcc2)
3. [Calculation Principles: Current vs. Power](#3-calculation-principles-current-vs-power)
4. [Detailed Command and Field Specification](#4-detailed-command-and-field-specification)
   - [`read-status`](#read-status)
   - [`read-current`](#read-current)
   - [`set-current`](#set-current)
   - [`set-power`](#set-power)
   - [`read-currents`](#read-currents)
   - [`read-firmware`](#read-firmware)
   - [`read-system-flags`](#read-system-flags)
   - [`read-modbus-settings`](#read-modbus-settings)
   - [`enable`](#enable)
   - [`disable`](#disable)
   - [`wake-up`](#wake-up)
   - [`reset`](#reset)
   - [`read-all`](#read-all)
   - [`daemon`](#daemon)

---

## 1. Glossary and Technical Abbreviations

| Abbreviation / Term | Full Name | Technical Explanation and Function |
|---|---|---|
| **CP** | **Control Pilot** | Signal contact per IEC 61851-1 in the Type 2 connector. Wallbox and EV communicate via a 1 kHz PWM signal. The voltage level indicates operating state (+12V = no EV, +9V = EV connected, +6V = charging request), while the duty cycle defines the maximum allowable charging current. |
| **CS / PP** | **Cable Sensing / Proximity Pilot** | Resistor coding between PP and PE in the charging cable. The wallbox detects the cable's current-carrying capacity (e.g. 13A, 20A, 32A). |
| **PWM** | **Pulse Width Modulation** | A square-wave signal of constant frequency (1 kHz) whose pulse width (duty cycle) is variable. |
| **Duty Cycle** | **Pulse Ratio (Tastgrad)** | Percentage of the high phase relative to the period length, expressed in tenths of a percent ($0.1\,\%$ steps). Formula: $\text{Current (A)} = \text{Duty Cycle} \times 0.06$. $267$ corresponds to $26.7\,\% = 16\,\text{A}$. $1000$ ($100\,\%$) means charging is disabled. |
| **EV** | **Electric Vehicle** | The connected electric vehicle and its On-Board Charger (OBC). |
| **EVSE** | **Electric Vehicle Supply Equipment** | The charging station / wallbox infrastructure. |
| **EVCC** | **Electric Vehicle Charge Controller** | The charge control board installed inside the wallbox (ABL EVCC2 or EVCC3 series). |
| **PCBA** | **Printed Circuit Board Assembly** | The populated controller board. The version code (e.g. `160307` for March 7, 2016) identifies the hardware revision. |
| **RDC-MD** | **Residual Direct Current - Monitoring Device** | Built-in electronic DC residual current detector. Trips at smooth DC residual current $\ge 6\,\text{mA}$ and opens all contactor poles, preventing saturation of upstream Type A RCDs. |
| **Welding Detection** | **Contactor Monitor** | Safety check inspecting the switching contacts of the main power contactor after shutdown to detect stuck or welded contacts. |
| **LRC** | **Longitudinal Redundancy Check** | Checksum algorithm used in Modbus ASCII: sum of all data bytes modulo 256, two's complement inverted (`wrapping_neg`), hex-encoded as 2 ASCII characters. |
| **8E1** | **Serial Data Format** | 8 data bits, Even parity, 1 stop bit at 38400 baud. |

---

## 2. Status Codes (IEC 61851-1 & ABL EVCC2)

The ABL eMH1 reports its exact operating position via the status byte in register `0x0004` (and byte 1 of register `0x002E`):

| Hex Code | IEC | Designation | Technical Description |
|:---:|:---:|:---|:---|
| **`0xA1`** | **A** | **Waiting for EV** | Standby: No vehicle connected. CP voltage is steady +12 V. |
| **`0xB1`** | **B** | **EV is asking for charging** | Vehicle connected, but charging permission not yet enabled. CP pulled down to +9 V by vehicle. Contactor open. |
| **`0xB2`** | **B** | **EV has permission to charge** | Vehicle connected and charge released. PWM active, but vehicle not yet drawing power. |
| **`0xC2`** | **C** | **EV is charging** | Active charging session. Vehicle pulls CP down to +6 V. Contactor closed, current flowing. |
| **`0xC3`** | **C** | **Reduced current (F16/F17)** | Active charging, but charging current throttled internally due to fault condition F16/F17. |
| **`0xC4`** | **C** | **Reduced current (Imbalance)** | Active charging, but current throttled due to phase load imbalance restriction (F15). |
| **`0xE0`** | **E** | **Outlet disabled** | Outlet disabled / software locked (CP = 0 V). |
| **`0xE1`** | **E** | **Production test** | Factory and production diagnosis mode active. |
| **`0xE2`** | **E** | **EVCC setup mode** | Controller is in configuration setup mode. |
| **`0xE3`** | **E** | **Bus idle** | Bus quiet / no master communications. |
| **`0xF1`** | **F** | **Welding detected** | Critical fault: Contactor contacts failed to open (welded contacts). |
| **`0xF2`** | **F** | **Internal error** | Internal hardware or firmware error in charge controller. |
| **`0xF3`** | **F** | **DC residual current detected** | Safety trip: RDC-MD detected smooth DC residual current $\ge 6\,\text{mA}$. |
| **`0xF4`** | **F** | **Communication timeout** | Upstream communication watchdog expired (loss of controller link). |
| **`0xF5`** | **F** | **Socket lock failed** | Type 2 locking motor actuator failed to engage or disengage. |
| **`0xF6`** | **F** | **CS out of range** | Resistor value measured on PP/CS cable coding line is invalid. |
| **`0xF7`** | **D** | **State D requested** | Vehicle requested external room ventilation (battery off-gassing). |
| **`0xF8`** | **F** | **CP out of range** | Control pilot voltage signal outside allowable IEC limits. |
| **`0xF9`** | **F** | **Overcurrent detected** | Current draw exceeds maximum configured rating. |
| **`0xFA`** | **F** | **Temperature limits** | Operating temperature limits exceeded. |
| **`0xFB`** | **F** | **Contactor opened error** | Main contactor opened unexpectedly during active charging. |

---

## 3. Calculation Principles: Current vs. Power

- **Amperes (`set-current`) is always the current per phase:**
  Allowable range: `0` (halt / stop charging) or `6` to `80` Amperes.
- **Watts (`set-power`) is always the total power across 3 phases (at $3 \times 230\,\text{V}$):**
  $$\text{Current per phase (A)} = \text{round}\left(\frac{\text{Power (W)}}{3 \times 230\,\text{V}}\right)$$
  - `11000 W` $\rightarrow$ $11000 / 690 = 15.94 \approx \mathbf{16\,\text{A}}$ per phase ($11,040\,\text{W}$)
  - `22000 W` $\rightarrow$ $22000 / 690 = 31.88 \approx \mathbf{32\,\text{A}}$ per phase ($22,080\,\text{W}$)
  - `0 W` $\rightarrow$ $\mathbf{0\,\text{A}}$ (charging stopped)

---

## 4. Detailed Command and Field Specification

### `read-status`
Reads the current IEC 61851-1 charging status from register `0x0004` (or `0x002E`).

- **Formatted console output:**
  ```text
  State A (0x00A1): Waiting for EV (Standby)
  ```
- **JSON output:**
  ```json
  "State A (0x00A1): Waiting for EV (Standby)"
  ```
- **Field breakdown:**
  - `State <Letter>`: Primary IEC 61851-1 state letter (`A`, `B`, `C`, `D`, `E`, `F`).
  - `(0x00..)`: Hexadecimal raw status code from the ABL EVCC2 controller.
  - Description: Human-readable explanation of current state.

---

### `read-current`
Reads the current limit setting from register `0x0014`.

- **Formatted console output:**
  ```text
  Configured Charging Current: 16.0A (Duty Cycle: 26.7%)
  ```
- **JSON output:**
  ```json
  {
    "current_amperes": 16.0,
    "duty_cycle_percent": 26.7,
    "duty_cycle_permille": 267,
    "enabled": true
  }
  ```
- **Field definitions:**
  - `current_amperes` (Float, unit: `A`): Calculated current limit per phase ($267 \times 0.06 = 16.02\,\text{A}$).
  - `duty_cycle_percent` (Float, unit: `%`): PWM duty cycle in percent ($26.7\,\%$).
  - `duty_cycle_permille` (Integer, unit: `‰`): Raw value of register `0x0014` in tenths of a percent ($267$). $1000$ denotes charging disabled.
  - `enabled` (Boolean): `true` if current limit $\ge 6\,\text{A}$ (charge release active); `false` if $0\,\text{A}$ (charge release revoked).

---

### `set-current <AMPERES> [--force]`
Sets the charging current limit per phase in register `0x0014`.

- **Arguments:**
  - `<AMPERES>`: Integer (`0` or `6`..`80`). Current in Amperes per phase.
  - `-f, --force`: Bypasses confirmation prompt when setting `0A`.
- **Output:**
  ```text
  Charging current successfully set to: 16A per phase (Total 3-phase power: 11040W)
  ```

---

### `set-power <WATTS> [--force]`
Calculates current per phase from 3-phase power in Watts and writes to register `0x0014`.

- **Arguments:**
  - `<WATTS>`: Integer. Total 3-phase power in Watts (at 230V).
  - `-f, --force`: Bypasses confirmation prompt when setting `0W`.
- **Output:**
  ```text
  Charging power 11000W (3-phase) successfully converted and set to: 16A per phase (effective power: 11040W).
  ```

---

### `read-currents`
Reads active phase currents and calculates instantaneous active power from register `0x002E`.

- **Formatted console output:**
  ```text
  Current L1: 15.8 A
  Current L2: 15.9 A
  Current L3: 15.7 A
  Total Active Power: 10902.0 W
  ```
- **JSON output:**
  ```json
  {
    "current_l1": 15.8,
    "current_l2": 15.9,
    "current_l3": 15.7,
    "power_watts": 10902.0
  }
  ```
- **Field definitions:**
  - `current_l1`, `current_l2`, `current_l3` (Float, unit: `A`):
    Actual measured current for each phase with $0.1\,\text{A}$ resolution. Returns `0.0` when idle or when meter is not installed.
  - `power_watts` (Float, unit: `W`):
    Calculated total active power at $230\,\text{V}$ nominal voltage: $(I_{L1} + I_{L2} + I_{L3}) \times 230\,\text{V}$.

---

### `read-firmware`
Reads controller identification and feature flags from register `0x0001` (2 words / 4 bytes).

- **Formatted console output:**
  ```text
  Device ID: 1
  Firmware Revision: V4.1
  PCBA Version: 160307
  Internal Current Sensor: Present
  RDC-MD Sensor: Enabled
  Contactor Welding Monitor: Enabled
  Outlet State: Enabled
  ```
- **JSON output:**
  ```json
  {
    "current_sensor_present": true,
    "device_id": 1,
    "firmware_revision": "4.1",
    "outlet_enabled": true,
    "pcba_version": "160307",
    "rdc_md_enabled": true,
    "welding_detection_enabled": true
  }
  ```
- **Field definitions:**
  - `device_id` (Integer): Configured Modbus slave address (1..16).
  - `firmware_revision` (String): Controller firmware version (e.g. `"4.1"`).
  - `pcba_version` (String): Controller board hardware revision (e.g. `"160307"` for PCBA 160307).
  - `current_sensor_present` (Boolean): `true` if built-in current transformer sensors are detected.
  - `rdc_md_enabled` (Boolean): `true` if electronic 6mA DC residual current monitor is active.
  - `welding_detection_enabled` (Boolean): `true` if contactor contact welding monitoring is enabled.
  - `outlet_enabled` (Boolean): `true` if charging outlet is enabled in hardware.

---

### `read-system-flags`
Reads internal diagnostic and operational flags from register `0x0006` (2 words / 4 bytes).

- **Formatted console output:**
  ```text
  Raw Flags: 0x0000 (State Pointer: 0x04)
  B1->B2 Transition Disabled: false
  B1->B2 Disabled After Power-Cycle: false
  Upstream Communication Lost: false
  Load Imbalance Detected: false
  Overtemperature (>60°C): false
  Phase Current Meter Fault: false
  E0->A1 Transition Disabled: false
  A1->B1 Transition Disabled: false
  ```
- **JSON output:**
  ```json
  {
    "a1_b1_disabled": false,
    "b1_b2_disabled": false,
    "b1_b2_disabled_after_power_cycle": false,
    "e0_a1_disabled": false,
    "load_imbalance_detected": false,
    "overtemperature_detected": false,
    "phase_current_meter_fault": false,
    "raw_flags": 0,
    "state_pointer": 4,
    "upstream_comm_lost": false
  }
  ```
- **Field definitions:**
  - `raw_flags` (Integer): 16-bit bitmask representing the raw system flags.
  - `state_pointer` (Integer): Pointer index into internal state machine.
  - `b1_b2_disabled` (Boolean): `true` if automatic transition to B2 is inhibited (e.g. external enable switch open).
  - `b1_b2_disabled_after_power_cycle` (Boolean): `true` if manual re-enable is enforced after power disruption.
  - `upstream_comm_lost` (Boolean): `true` if communication watchdog expired (error F4 / BC6).
  - `load_imbalance_detected` (Boolean): `true` if phase imbalance $>20\,\text{A}$ is active (BC3).
  - `overtemperature_detected` (Boolean): `true` if internal operating temperature exceeds $60^\circ\text{C}$ (BC5).
  - `phase_current_meter_fault` (Boolean): `true` if current measurement hardware failed (BC4).
  - `e0_a1_disabled` / `a1_b1_disabled` (Boolean): State transitions blocked by configuration.

---

### `read-modbus-settings`
Reads serial communications parameters from register `0x0003`.

- **Formatted console output:**
  ```text
  Baud rate: 38400 Bd, Parity: Even, Stop bits: 1
  ```
- **JSON output:**
  ```json
  {
    "baud_rate": 38400,
    "parity": "Even",
    "stop_bits": 1
  }
  ```
- **Field definitions:**
  - `baud_rate` (Integer): Configured baud rate (`38400`).
  - `parity` (String): Parity scheme (`"Even"`, `"Odd"`, `"None"`).
  - `stop_bits` (Integer): Number of stop bits (`1` or `2`).

---

### `enable [AMPERES] [--power <WATTS>]`
Enables charging and writes current limit to register `0x0014`.

- **Options:**
  - `[AMPERES]`: Current in Amperes per phase (default: `16`).
  - `--power <WATTS>`: Total 3-phase power in Watts (e.g. `11000`), converted to Amperes per phase.

---

### `disable [--force]`
Revokes charging permission (writes `0` to register `0x0014`), causing the contactor to open and stopping charging immediately.

- **Options:**
  - `-f, --force`: Suppresses interactive confirmation prompt.

---

### `wake-up`
Executes standard Control Pilot cycling to wake up sleeping vehicle chargers:
1. Writes `0xE0E0` to register `0x0005` (State E0: CP voltage pulls to 0 V).
2. Waits 3 seconds.
3. Writes `0xA1A1` to register `0x0005` (State A1: CP voltage restores to +12 V).

---

### `reset [--force]`
Sends reset command `0x5A5A` to register `0x0005` to restart the EVCC2 controller.

- **Options:**
  - `-f, --force`: Suppresses warning prompt.

---

### `read-all`
Combines `read-firmware`, `read-system-flags`, and measurements in a single call.

- **JSON output:**
  ```json
  {
    "firmware": {
      "current_sensor_present": true,
      "device_id": 1,
      "firmware_revision": "4.1",
      "outlet_enabled": true,
      "pcba_version": "160307",
      "rdc_md_enabled": true,
      "welding_detection_enabled": true
    },
    "measurements": {
      "configured_current_amperes": 16.0,
      "current_l1_amperes": 0.0,
      "current_l2_amperes": 0.0,
      "current_l3_amperes": 0.0,
      "duty_cycle_permille": 267,
      "duty_cycle_reduced": false,
      "en1_closed": true,
      "en2_closed": false,
      "ev_connected": false,
      "status": "WaitingForEv",
      "total_power_watts": 0.0
    },
    "system_flags": {
      "a1_b1_disabled": false,
      "b1_b2_disabled": false,
      "b1_b2_disabled_after_power_cycle": false,
      "e0_a1_disabled": false,
      "load_imbalance_detected": false,
      "overtemperature_detected": false,
      "phase_current_meter_fault": false,
      "raw_flags": 0,
      "state_pointer": 4,
      "upstream_comm_lost": false
    }
  }
  ```

---

### `daemon` (Console & MQTT)
Polls the wallbox periodically at the configured interval (default: 2 seconds).

- **Console mode:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon console
  ```
- **MQTT mode:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon mqtt --mqtt-config mqtt.yaml
  ```
- **Published MQTT topics:**
  - `<topic>/status`: Character (`A`, `B`, `C`, `D`, `E`, `F`)
  - `<topic>/status_code`: Hex code (`0x00A1`, `0x00B1`, `0x00C2`, etc.)
  - `<topic>/status_description`: Human-readable status description
  - `<topic>/ev_connected`: Boolean (`true`/`false`)
  - `<topic>/configured_current`: Target charging current in Amperes per phase
  - `<topic>/enabled`: Boolean (`true` if current $\ge 6\,\text{A}$)
  - `<topic>/current_l1`, `current_l2`, `current_l3`: Measured phase currents in Amperes
  - `<topic>/power`: Total calculated active power in Watts
  - `<topic>/JSON`: Full JSON measurement object

- **Bidirectional remote control (when `allow_control: true` in `mqtt.yaml`):**
  - `<topic>/set/current`: Sets current in **Amperes per phase** (e.g. `16` or `0`). Capped by `max_current`.
  - `<topic>/set/power`: Sets power in **Watts for 3 phases** (e.g. `11000` or `0`). Capped by `max_current`.
  - `<topic>/set/enable`: `"true"` (enables charging at `max_current` per phase) or `"false"` (stops charging).
  - `<topic>/set/wake_up`: `"true"` (triggers CP wake-up sequence E0 -> A1).
  - `<topic>/set/reset`: `"true"` (restarts the charge controller).
