# ABL eMH1 (EVCC2/3) Befehls-, Werte- und Register-Referenz

Dieses Dokument bietet eine vollständige und detaillierte Dokumentation aller Kommandos, einstellbaren Werte, Rückgabefelder sowie aller verwendeten Abkürzungen und technischen Begriffe für die **ABL eMH1** (und eMH2 / EVCC2/3) Wallboxen.

---

## Inhaltsverzeichnis
1. [Glossar und Abkürzungen](#1-glossar-und-abkürzungen)
2. [Statuscodes nach IEC 61851-1 und ABL EVCC2](#2-statuscodes-nach-iec-61851-1-und-abl-evcc2)
3. [Berechnungsgrundlagen: Stromstärke vs. Leistung](#3-berechnungsgrundlagen-stromstärke-vs-leistung)
4. [Detaillierte Befehlsreferenz](#4-detaillierte-befehlsreferenz)
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

## 1. Glossar und Abkürzungen

| Abkürzung / Begriff | Vollständige Bezeichnung / Erklärung |
|---|---|
| **CP** | **Control Pilot** (Signalkontakt im Typ-2-Stecker nach IEC 61851-1). Überträgt ein 1 kHz PWM-Rechtecksignal zwischen Wallbox und Fahrzeug zur Kommunikation (Zustandserkennung über Spannungspegel +12V, +9V, +6V, 0V, -12V und Stromvorgabe über den Tastgrad). |
| **CS / PP** | **Cable Sensing / Proximity Pilot** (Widerstandskodierung im Ladekabel). Dient der Erkennung der maximalen Strombelastbarkeit des verwendeten Typ-2-Ladekabels (z. B. 13A, 20A, 32A). |
| **PWM** | **Pulse Width Modulation** (Pulsweitenmodulation). Die Rechteckwelle auf der CP-Leitung. |
| **Duty Cycle / Tastgrad** | Das Verhältnis der Einschaltzeit zur Periodendauer des PWM-Signals in 0,1 %-Schritten (z. B. $267 = 26{,}7\,\% \rightarrow 16\,\text{A}$). |
| **EV / EVSE** | **Electric Vehicle** (Elektrofahrzeug) / **Electric Vehicle Supply Equipment** (Ladeeinrichtung / Wallbox). |
| **EVCC** | **Electric Vehicle Charge Controller** (die in der ABL Wallbox verbaute Steuerplatine der Serie EVCC2 bzw. EVCC3). |
| **PCBA** | **Printed Circuit Board Assembly** (bestückte Leiterplatte / Hardware-Revision der ABL Steuerung, z. B. `160307`). |
| **RDC-MD** | **Residual Direct Current - Monitoring Device** (Gleichfehlerstrom-Überwachung). Erkennt DC-Fehlerströme $\ge 6\,\text{mA}$ und schaltet die Wallbox sicher ab, wodurch ein teurer FI-Schutzschalter Typ B im Sicherungskasten entfällt. |
| **Welding Detection** | **Schütz-Überwachung (Kontaktverschweißung)**. Prüft nach dem Ladeende, ob die Leistungskontakte des Schützes ordnungsgemäß geöffnet haben oder verschweißt sind. |
| **LRC** | **Longitudinal Redundancy Check** (Längsparitätsprüfung). Prüfsummenverfahren für Modbus ASCII (2 Hex-Zeichen am Ende jedes Frames). |
| **8E1** | Serielles Datenformat: **8** Datenbits, **E**ven Parity (gerade Parität), **1** Stopbit bei 38400 Baud. |

---

## 2. Statuscodes nach IEC 61851-1 und ABL EVCC2

Die Wallbox meldet ihren Betriebszustand in den Registern `0x0004` und `0x002E`:

| Hex-Code | IEC-Zustand | Bedeutung / Beschreibung |
|:---:|:---:|:---|
| **`0xA1`** | **A** | **Standby / Kein Fahrzeug verbunden** (Wartet auf Anschluss eines Fahrzeugs, CP = +12V statisch). |
| **`0xB1`** | **B** | **Fahrzeug verbunden, wartet auf Ladefreigabe** (Fahrzeug erkannt, Schütz noch offen, CP = +9V). |
| **`0xB2`** | **B** | **Fahrzeug verbunden, Ladefreigabe erteilt** (Wallbox bereit, wartet auf Ladeanforderung des Autos). |
| **`0xC2`** | **C** | **Ladevorgang aktiv** (Fahrzeug lädt, Hauptschütz geschlossen, CP = +6V mit aktivem PWM). |
| **`0xC3`** | **C** | **Laden aktiv mit reduziertem Strom** (Fehler F16 oder F17 aktiv). |
| **`0xC4`** | **C** | **Laden aktiv mit reduziertem Strom** (Schieflast-Begrenzung F15 aktiv). |
| **`0xE0`** | **E** | **Ladepunkt deaktiviert** (Outlet disabled / Software-Sperre). |
| **`0xE1`** | **E** | **Produktionstest** (Fertigungsdiagnose). |
| **`0xE2`** | **E** | **Setup-Modus** (Konfigurationsmodus der EVCC). |
| **`0xE3`** | **E** | **Bus Idle** (Keine Buskommunikation). |
| **`0xF1`** | **F** | **Schützfehler / Welding** (Schützkontakt unerwünscht geschlossen). |
| **`0xF2`** | **F** | **Interner Steuerungsfehler** (Controller-Hardwarefehler). |
| **`0xF3`** | **F** | **DC-Fehlerstrom erkannt** (RDC-MD hat ausgelöst, DC $\ge 6\,\text{mA}$). |
| **`0xF4`** | **F** | **Kommunikations-Timeout** (Keine Upstream-Nachricht empfangen). |
| **`0xF5`** | **F** | **Steckerverriegelung fehlgeschlagen** (Motor der Typ-2-Buchse blockiert). |
| **`0xF6`** | **F** | **CS-Widerstand unzulässig** (Kabelkodierung fehlerhaft oder Kabel defekt). |
| **`0xF7`** | **D** | **Zustand D angefordert** (Fahrzeug verlangt externe Raumlüftung, z. B. bei Bleiakkus). |
| **`0xF8`** | **F** | **CP-Spannung außerhalb des Toleranzbereichs**. |
| **`0xF9`** | **F** | **Überstrom erkannt**. |
| **`0xFA`** | **F** | **Übertemperatur** (Gehäuseinnentemperatur überschritten). |
| **`0xFB`** | **F** | **Schützkontakt unerwünscht geöffnet**. |

---

## 3. Berechnungsgrundlagen: Stromstärke vs. Leistung

- **Ampere (`set-current`) gilt pro Phase:**
  Die Norm IEC 61851-1 definiert das PWM-Signal immer als maximalen Strom **je Phase**.
  Zulässige Werte: `0` (Halt/Ladestopp) oder `6` bis `80` Ampere.
- **Watt (`set-power`) gilt für 3 Phasen:**
  Für dreiphasiges Laden ($3 \times 230\,\text{V} = 690\,\text{V}$) wird die Wattangabe automatisch umgerechnet:
  $$\text{Stromstärke (A)} = \text{round}\left(\frac{\text{Leistung in Watt}}{3 \times 230\,\text{V}}\right)$$
  - `11000 W` $\rightarrow$ `16 A` (entspricht $16\,\text{A} \times 3 \times 230\,\text{V} = 11.040\,\text{W}$)
  - `22000 W` $\rightarrow$ `32 A` (entspricht $32\,\text{A} \times 3 \times 230\,\text{V} = 22.080\,\text{W}$)
  - `0 W` $\rightarrow$ `0 A` (Ladestopp)

---

## 4. Detaillierte Befehlsreferenz

### `read-status`
Liest den aktuellen IEC 61851-1 Ladestatus der Wallbox aus Register `0x0004` (oder `0x002E`).

- **Eingabe:** Keine Argumente.
- **Konsolen-Ausgabe (formatiert):**
  ```text
  State A (0x00A1): Waiting for EV (Standby)
  ```
- **JSON-Ausgabe:**
  ```json
  "State A (0x00A1): Waiting for EV (Standby)"
  ```

---

### `read-current`
Liest die aktuell in Register `0x0014` hinterlegte Stromvorgabe (PWM-Tastgrad) aus.

- **Eingabe:** Keine Argumente.
- **Konsolen-Ausgabe (formatiert):**
  ```text
  Configured Charging Current: 16.0A (Duty Cycle: 26.7%)
  ```
- **JSON-Ausgabe:**
  ```json
  {
    "current_amperes": 16.0,
    "duty_cycle_percent": 26.7,
    "duty_cycle_permille": 267,
    "enabled": true
  }
  ```
- **Bedeutung der Felder:**
  - `current_amperes`: Berechnete Stromstärke in Ampere pro Phase ($267 \times 0{,}06 = 16{,}02\,\text{A}$).
  - `duty_cycle_permille`: Rohwert des Registers in 0,1 % ($1000$ = Ladevorgang deaktiviert/gesperrt).
  - `enabled`: `true`, wenn Strom $\ge 6\,\text{A}$ beträgt (Ladefreigabe aktiv).

---

### `set-current <AMPERES> [--force]`
Stellt das Stromlimit pro Phase in Register `0x0014` ein.

- **Parameter:**
  - `<AMPERES>`: Ganzzahliger Wert: `0` (Halt/Freigabe entzogen) oder `6` bis `80` Ampere pro Phase.
  - `-f, --force`: Überspringt die Sicherheitsbestätigung bei `0A`.
- **Sicherheitsabfrage:**
  Beim Setzen von `0A` fragt das Tool interaktiv nach (`Continue? [y/N]`), um ein versehentliches Abbrechen aktiver Ladevorgänge zu verhindern.
- **Konsolen-Ausgabe:**
  ```text
  Charging current successfully set to: 16A per phase (Total 3-phase power: 11040W)
  ```

---

### `set-power <WATTS> [--force]`
Berechnet aus der dreiphasigen Leistung in Watt die entsprechende Stromstärke pro Phase und schreibt sie in Register `0x0014`.

- **Parameter:**
  - `<WATTS>`: Ganzzahliger Leistungswert in Watt für 3 Phasen (z. B. `11000` für 11 kW, `0` zum Stoppen).
  - `-f, --force`: Überspringt die Sicherheitsbestätigung bei `0W`.
- **Konsolen-Ausgabe:**
  ```text
  Charging power 11000W (3-phase) successfully converted and set to: 16A per phase (effective power: 11040W).
  ```

---

### `read-currents`
Liest die tatsächlich gemessenen Ströme aller 3 Phasen aus Register `0x002E` aus und berechnet die momentane Gesamtwirkleistung in Watt.

- **Eingabe:** Keine Argumente.
- **Konsolen-Ausgabe (formatiert):**
  ```text
  Current L1: 15.8 A
  Current L2: 15.9 A
  Current L3: 15.7 A
  Total Active Power: 10902.0 W
  ```
- **JSON-Ausgabe:**
  ```json
  {
    "current_l1": 15.8,
    "current_l2": 15.9,
    "current_l3": 15.7,
    "power_watts": 10902.0
  }
  ```

---

### `read-firmware`
Liest Hardware- und Firmware-Informationen aus Register `0x0001` (2 Wörter / 4 Bytes).

- **Eingabe:** Keine Argumente.
- **Konsolen-Ausgabe (formatiert):**
  ```text
  Device ID: 1
  Firmware Revision: V4.1
  PCBA Version: 160307
  Internal Current Sensor: Present
  RDC-MD Sensor: Enabled
  Contactor Welding Monitor: Enabled
  Outlet State: Enabled
  ```
- **JSON-Ausgabe:**
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

---

### `read-system-flags`
Liest interne System-Flags und den Zustandszeiger aus Register `0x0006` (2 Wörter / 4 Bytes).

- **Eingabe:** Keine Argumente.
- **Konsolen-Ausgabe (formatiert):**
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
- **JSON-Ausgabe:**
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

---

### `read-modbus-settings`
Liest die konfigurierten seriellen Schnittstellen-Parameter aus Register `0x0003` (1 Wort / 2 Bytes).

- **Eingabe:** Keine Argumente.
- **Konsolen-Ausgabe (formatiert):**
  ```text
  Baud rate: 38400 Bd, Parity: Even, Stop bits: 1
  ```
- **JSON-Ausgabe:**
  ```json
  {
    "baud_rate": 38400,
    "parity": "Even",
    "stop_bits": 1
  }
  ```

---

### `enable [AMPERES] [--power <WATTS>]`
Aktiviert die Ladefreigabe für die Wallbox.

- **Optionen:**
  - `[AMPERES]`: Optionale Angabe in Ampere pro Phase (Standard: `16`).
  - `--power <WATTS>`: Optional als dreiphasige Wattangabe (z. B. `--power 11000`).
- **Konsolen-Ausgabe:**
  ```text
  Charging successfully enabled at 16A per phase.
  ```

---

### `disable [--force]`
Entzieht der Wallbox die Ladefreigabe (setzt Stromvorgabe auf 0A / Duty Cycle 1000), wodurch das Hauptschütz abfällt und der Ladevorgang sofort stoppt.

- **Optionen:**
  - `-f, --force`: Überspringt die Sicherheitsbestätigung.
- **Konsolen-Ausgabe:**
  ```text
  Charging successfully disabled / halted.
  ```

---

### `wake-up`
Weckt schlafende Elektrofahrzeuge (On-Board-Charger) über die standardisierte Control-Pilot-Sequenz auf:
1. Schreibt `0xE0E0` in Register `0x0005` (Zustand E0: CP-Spannung wird kurzzeitig auf 0V geschaltet).
2. Wartet 3 Sekunden Pause.
3. Schreibt `0xA1A1` in Register `0x0005` (Zustand A1: CP-Spannung wird wieder auf +12V gesetzt).
Das Fahrzeug erkennt die Signaländerung wie ein erneutes Einstecken des Ladekabels und startet den Lader neu.

---

### `reset [--force]`
Startet den Mikrocontroller der ABL EVCC2 neu (schreibt `0x5A5A` in Register `0x0005`).

- **Optionen:**
  - `-f, --force`: Überspringt die Sicherheitswarnung.
- **Konsolen-Ausgabe:**
  ```text
  Reset command acknowledged.
  ```

---

### `read-all`
Kombiniert die Abfragen aus `read-firmware`, `read-system-flags` und `read-currents` in einem einzigen Diagnosebericht.

- **JSON-Ausgabe:**
  ```json
  {
    "firmware": {
      "device_id": 1,
      "firmware_revision": "4.1",
      "pcba_version": "160307",
      "current_sensor_present": true,
      "rdc_md_enabled": true,
      "welding_detection_enabled": true,
      "outlet_enabled": true
    },
    "system_flags": {
      "raw_flags": 0,
      "state_pointer": 4,
      "b1_b2_disabled": false,
      "b1_b2_disabled_after_power_cycle": false,
      "upstream_comm_lost": false,
      "load_imbalance_detected": false,
      "overtemperature_detected": false,
      "phase_current_meter_fault": false,
      "e0_a1_disabled": false,
      "a1_b1_disabled": false
    },
    "measurements": {
      "status": "WaitingForEv",
      "ev_connected": false,
      "duty_cycle_reduced": false,
      "en1_closed": true,
      "en2_closed": false,
      "duty_cycle_permille": 267,
      "configured_current_amperes": 16.0,
      "current_l1_amperes": 0.0,
      "current_l2_amperes": 0.0,
      "current_l3_amperes": 0.0,
      "total_power_watts": 0.0
    }
  }
  ```

---

### `daemon` (Console & MQTT)
Führt eine kontinuierliche Abfrage im konfigurierten Intervall durch (z. B. alle 2 Sekunden).

- **Console-Modus:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon console
  ```
- **MQTT-Modus:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon mqtt --mqtt-config mqtt.yaml
  ```
- **MQTT-Steuerung (`allow_control: true` in `mqtt.yaml`):**
  Wenn `allow_control` aktiviert ist, können folgende MQTT-Befehle an `<topic>/set/...` gesendet werden:
  - `<topic>/set/current`: Stromstärke in Ampere pro Phase (z. B. `16` oder `0`). Gedeckelt auf `max_current`.
  - `<topic>/set/power`: Leistung in Watt für 3 Phasen (z. B. `11000` oder `0`). Gedeckelt auf `max_current`.
  - `<topic>/set/enable`: `"true"` (aktiviert das Laden mit dem konfigurierten `max_current`) oder `"false"` (deaktiviert das Laden).
  - `<topic>/set/wake_up`: `"true"` (löst die Wake-Up-Sequenz E0 -> A1 aus).
  - `<topic>/set/reset`: `"true"` (startet den Ladecontroller neu).
