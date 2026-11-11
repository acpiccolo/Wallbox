# ABL eMH1 (EVCC2/3) Befehls-, Werte- und Register-Referenz

Dieses Dokument bietet eine vollständige, feldgenaue Dokumentation aller Kommandos, einstellbaren Werte, Rückgabefelder sowie aller verwendeten Abkürzungen und technischen Begriffe für **ABL eMH1** (und eMH2 / EVCC2/3) Wallboxen.

---

## Inhaltsverzeichnis
1. [Glossar und Abkürzungen](#1-glossar-und-abkürzungen)
2. [Statuscodes nach IEC 61851-1 und ABL EVCC2](#2-statuscodes-nach-iec-61851-1-und-abl-evcc2)
3. [Berechnungsgrundlagen: Stromstärke vs. Leistung](#3-berechnungsgrundlagen-stromstärke-vs-leistung)
4. [Detaillierte Befehls- und Feldspezifikation](#4-detaillierte-befehls--und-feldspezifikation)
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

| Abkürzung / Begriff | Vollständige Bezeichnung | Technische Erklärung und Funktion |
|---|---|---|
| **CP** | **Control Pilot** | Signalkontakt nach IEC 61851-1 im Typ-2-Stecker. Über ein 1-kHz-PWM-Signal kommunizieren Wallbox und Fahrzeug. Das Spannungsniveau signalisiert den Zustand (+12V = kein Auto, +9V = Auto verbunden, +6V = Ladeanforderung), während das Tastverhältnis (Duty Cycle) die maximal erlaubte Stromstärke vorgibt. |
| **CS / PP** | **Cable Sensing / Proximity Pilot** | Widerstandskodierung zwischen PP und PE im Ladekabel. Die Wallbox erkennt darüber die Stromtragfähigkeit des Kabels (z. B. 13A, 20A, 32A). |
| **PWM** | **Pulsweitenmodulation** | Rechtecksignal konstanter Frequenz (1 kHz), dessen Pulsbreite (Tastgrad) variabel ist. |
| **Duty Cycle / Tastgrad** | **Pulsweitenverhältnis** | Anteil der High-Phase an der Periodendauer in Promille ($0{,}1\,\%$-Schritten). Formel: $\text{Strom (A)} = \text{Tastgrad} \times 0{,}06$. $267$ entspricht $26{,}7\,\% = 16\,\text{A}$. $1000$ ($100\,\%$) bedeutet Laden gesperrt. |
| **EV** | **Electric Vehicle** | Das angeschlossene Elektrofahrzeug bzw. dessen On-Board-Charger (OBC). |
| **EVSE** | **Electric Vehicle Supply Equipment** | Die Ladeinfrastruktur (Wallbox / Ladestation). |
| **EVCC** | **Electric Vehicle Charge Controller** | Die in der Wallbox verbaute Steuerelektronik (ABL EVCC2 bzw. EVCC3). |
| **PCBA** | **Printed Circuit Board Assembly** | Die bestückte Steuerplatine der Wallbox. Der Code (z. B. `160307` für 07.03.2016) identifiziert das Hardware-Layout. |
| **RDC-MD** | **Residual Direct Current - Monitoring Device** | Integrierte Gleichfehlerstrom-Überwachung. Löst bei einem glatten DC-Fehlerstrom von $\ge 6\,\text{mA}$ aus und trennt das Ladeschütz allpolig. Verhindert die Vormagnetisierung bauseitiger Typ-A-FI-Schutzschalter. |
| **Welding Detection** | **Schütz-Verschweißungserkennung** | Sicherheitsprüfung der Schaltkontakte des Leistungsschützes nach dem Abschalten. Erkennt, ob Kontakte kleben oder verschweißt sind. |
| **LRC** | **Longitudinal Redundancy Check** | Prüfsummenalgorithmus für Modbus ASCII: Summe aller Datenbytes (Modulo 256), negiert im Zweierkomplement (`wrapping_neg`), hexadezimal kodiert. |
| **8E1** | **Serielles Datenformat** | 8 Datenbits, Even Parity (gerade Parität), 1 Stopbit bei 38400 Baud. |

---

## 2. Statuscodes nach IEC 61851-1 und ABL EVCC2

Die ABL eMH1 liefert im Status-Byte (Register `0x0004` bzw. Byte 1 in `0x002E`) exakte Zustände:

| Hex-Code | IEC | Bezeichnung | Technische Beschreibung |
|:---:|:---:|:---|:---|
| **`0xA1`** | **A** | **Waiting for EV** | Standby: Kein Fahrzeug angeschlossen. CP-Spannung liegt bei statischen +12 V. |
| **`0xB1`** | **B** | **EV is asking for charging** | Fahrzeug angeschlossen, aber noch keine Ladefreigabe aktiv. CP wird vom Fahrzeug auf +9 V gezogen. Schütz ist offen. |
| **`0xB2`** | **B** | **EV has permission to charge** | Fahrzeug angeschlossen und Freigabe erteilt. PWM aktiv, aber Fahrzeug fordert noch keine Energie an. |
| **`0xC2`** | **C** | **EV is charging** | Ladevorgang aktiv. Fahrzeug zieht CP auf +6 V. Schütz ist geschlossen, Strom fließt. |
| **`0xC3`** | **C** | **Reduced current (F16/F17)** | Laden aktiv, Stromstärke jedoch wegen Fehlerzustand F16/F17 intern gedrosselt. |
| **`0xC4`** | **C** | **Reduced current (Imbalance)** | Laden aktiv, Stromstärke wegen Schieflast-Begrenzung (F15) gedrosselt. |
| **`0xE0`** | **E** | **Outlet disabled** | Ladepunkt softwareseitig gesperrt / deaktiviert (CP = 0 V). |
| **`0xE1`** | **E** | **Production test** | Fertigungs- und Endprüfungsmodus aktiv. |
| **`0xE2`** | **E** | **EVCC setup mode** | Controller befindet sich im Konfigurationsmodus. |
| **`0xE3`** | **E** | **Bus idle** | Busruhe / keine Master-Kommunikation. |
| **`0xF1`** | **F** | **Welding detected** | Schwerer Fehler: Schützkontakte öffnen nicht (Kontaktverschweißung). |
| **`0xF2`** | **F** | **Internal error** | Interner Hardware- oder Firmwarefehler des Controllers. |
| **`0xF3`** | **F** | **DC residual current detected** | Schutzabschaltung: RDC-MD hat DC-Fehlerstrom $\ge 6\,\text{mA}$ detektiert. |
| **`0xF4`** | **F** | **Communication timeout** | Upstream-Timeout überschritten (Verbindungsabbruch zum Managementsystem). |
| **`0xF5`** | **F** | **Socket lock failed** | Verriegelungsaktuator der Typ-2-Steckdose konnte nicht ein- oder ausfahren. |
| **`0xF6`** | **F** | **CS out of range** | Der über PP/CS gemessene Kabelkodierwiderstand ist unplausibel. |
| **`0xF7`** | **D** | **State D requested** | Fahrzeug verlangt externe Belüftung (Ausgasung bei Blei-Akkus). |
| **`0xF8`** | **F** | **CP out of range** | Signalpegel auf der Control-Pilot-Leitung außerhalb des IEC-Toleranzbereichs. |
| **`0xF9`** | **F** | **Overcurrent detected** | Ladeüberstrom erkannt (Fahrzeug zieht mehr Strom als signalisiert). |
| **`0xFA`** | **F** | **Temperature limits** | Gehäuse- oder Bauteilübertemperatur überschritten. |
| **`0xFB`** | **F** | **Contactor opened error** | Schütz öffnete unerwartet während des aktiven Ladevorgangs. |

---

## 3. Berechnungsgrundlagen: Stromstärke vs. Leistung

- **Ampere (`set-current`) ist immer der Strom pro Phase:**
  Zulässiger Wertebereich: `0` (Ladestopp) oder `6` bis `80` Ampere.
- **Watt (`set-power`) ist immer die Gesamtleistung für 3 Phasen (bei $3 \times 230\,\text{V}$):**
  $$\text{Ampere pro Phase} = \text{round}\left(\frac{\text{Leistung (W)}}{3 \times 230\,\text{V}}\right)$$
  - `11000 W` $\rightarrow$ $11000 / 690 = 15{,}94 \approx \mathbf{16\,\text{A}}$ pro Phase (entspricht $11.040\,\text{W}$)
  - `22000 W` $\rightarrow$ $22000 / 690 = 31{,}88 \approx \mathbf{32\,\text{A}}$ pro Phase (entspricht $22.080\,\text{W}$)
  - `0 W` $\rightarrow$ $\mathbf{0\,\text{A}}$ (Laden stoppen)

---

## 4. Detaillierte Befehls- und Feldspezifikation

### `read-status`
Liest den aktuellen IEC 61851-1 Ladestatus aus Register `0x0004` (bzw. `0x002E`).

- **Konsolen-Ausgabe:**
  ```text
  State A (0x00A1): Waiting for EV (Standby)
  ```
- **JSON-Ausgabe:**
  ```json
  "State A (0x00A1): Waiting for EV (Standby)"
  ```
- **Feldbeschreibung:**
  - `State <Buchstabe>`: Der IEC 61851-1 Hauptzustand (`A`, `B`, `C`, `D`, `E`, `F`).
  - `(0x00..)`: Der hexadezimale Rohstatuscode der ABL EVCC2 (siehe Abschnitt 2).
  - Textbeschreibung: Menschlich lesbare Erklärung des Betriebszustands.

---

### `read-current`
Liest das aktuell vorgegebene Stromlimit aus Register `0x0014`.

- **Konsolen-Ausgabe:**
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
- **Felddefinitionen:**
  - `current_amperes` (Float, Einheit: Ampere `A`):
    Der aus dem Tastgrad errechnete Soll-Ladestrom pro Phase ($267 \times 0{,}06 = 16{,}0\,\text{A}$).
  - `duty_cycle_percent` (Float, Einheit: `%`):
    PWM-Tastgrad in Prozent ($26{,}7\,\%$).
  - `duty_cycle_permille` (Integer, Einheit: `‰`):
    Rohwert aus Register `0x0014` in 0,1 %-Schritten ($267$). $1000$ bedeutet Ladevorgang deaktiviert.
  - `enabled` (Boolean):
    `true`, wenn der Sollstrom $\ge 6\,\text{A}$ beträgt (Ladefreigabe aktiv). `false`, wenn $0\,\text{A}$ (gesperrt).

---

### `set-current <AMPERES> [--force]`
Setzt das Stromlimit pro Phase in Register `0x0014`.

- **Argumente:**
  - `<AMPERES>`: Integer (`0` oder `6`..`80`). Stromstärke in Ampere pro Phase.
  - `-f, --force`: Überspringt die Bestätigungsabfrage beim Setzen von `0A`.
- **Konsolen-Ausgabe:**
  ```text
  Charging current successfully set to: 16A per phase (Total 3-phase power: 11040W)
  ```

---

### `set-power <WATTS> [--force]`
Berechnet aus der dreiphasigen Leistung in Watt die Stromstärke pro Phase und setzt diese in Register `0x0014`.

- **Argumente:**
  - `<WATTS>`: Integer. Gesamtleistung in Watt über 3 Phasen (bei 230V).
  - `-f, --force`: Überspringt die Bestätigungsabfrage bei `0W`.
- **Konsolen-Ausgabe:**
  ```text
  Charging power 11000W (3-phase) successfully converted and set to: 16A per phase (effective power: 11040W).
  ```

---

### `read-currents`
Liest die Phasenströme und berechnet die momentane Wirkleistung aus Register `0x002E`.

- **Konsolen-Ausgabe:**
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
- **Felddefinitionen:**
  - `current_l1`, `current_l2`, `current_l3` (Float, Einheit: Ampere `A`):
    Echte, vom internen Wandler gemessene Ströme der einzelnen Phasen mit $0{,}1\,\text{A}$ Auflösung. Liefert `0.0`, wenn kein Strom fließt oder kein Messwandler vorhanden ist.
  - `power_watts` (Float, Einheit: Watt `W`):
    Berechnete Gesamtwirkleistung bei $230\,\text{V}$ Netzspannung: $(I_{L1} + I_{L2} + I_{L3}) \times 230\,\text{V}$.

---

### `read-firmware`
Liest Identifikations- und Ausstattungsdaten aus Register `0x0001` (2 Wörter / 4 Bytes).

- **Konsolen-Ausgabe:**
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
- **Felddefinitionen:**
  - `device_id` (Integer): Konfigurierte Modbus-Slave-Adresse (1..16).
  - `firmware_revision` (String): Version der Controller-Firmware (z. B. `"4.1"`).
  - `pcba_version` (String): Hardware-Revision der Leiterplatte (z. B. `"160307"` für PCBA 160307).
  - `current_sensor_present` (Boolean): `true`, wenn die Wallbox über integrierte Phasenstrom-Messwandler verfügt.
  - `rdc_md_enabled` (Boolean): `true`, wenn die elektronische 6mA-DC-Fehlerstromerkennung aktiv ist.
  - `welding_detection_enabled` (Boolean): `true`, wenn Schützkontakt-Verschweißung automatisch überwacht wird.
  - `outlet_enabled` (Boolean): `true`, wenn der Ladepunkt betriebsbereit und hardwaremäßig freigegeben ist.

---

### `read-system-flags`
Liest interne Betriebs- und Diagnose-Flags aus Register `0x0006` (2 Wörter / 4 Bytes).

- **Konsolen-Ausgabe:**
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
- **Felddefinitionen:**
  - `raw_flags` (Integer): Rohwert der Systemflags als 16-Bit-Bitmaske.
  - `state_pointer` (Integer): Zeiger auf die aktuelle Position in der internen Zustandsmaschine.
  - `b1_b2_disabled` (Boolean): `true`, wenn automatische Ladefreigabe blockiert ist (z. B. durch externes Freigabesignal an Klemme EN1/EN2).
  - `b1_b2_disabled_after_power_cycle` (Boolean): `true`, wenn nach Spannungsunterbrechung stets eine manuelle Neu-Freigabe erzwungen wird.
  - `upstream_comm_lost` (Boolean): `true`, wenn der Kommunikations-Watchdog zu einem übergeordneten Master ausgelöst hat (Fehler F4/BC6).
  - `load_imbalance_detected` (Boolean): `true`, wenn eine unzulässige Schieflast von $>20\,\text{A}$ zwischen den Phasen festgestellt wurde (BC3).
  - `overtemperature_detected` (Boolean): `true`, wenn Gehäusetemperatur $60^\circ\text{C}$ bis $80^\circ\text{C}$ überschreitet (BC5).
  - `phase_current_meter_fault` (Boolean): `true`, wenn bei der Strommessung ein Sensorfehler vorliegt (BC4).
  - `e0_a1_disabled` / `a1_b1_disabled` (Boolean): Zustandsübergänge softwareseitig gesperrt.

---

### `read-modbus-settings`
Liest die Übertragungsparameter der seriellen Schnittstelle aus Register `0x0003`.

- **Konsolen-Ausgabe:**
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
- **Felddefinitionen:**
  - `baud_rate` (Integer): Aktuelle Schnittstellengeschwindigkeit in Baud (`38400`).
  - `parity` (String): Paritätsprüfung (`"Even"`, `"Odd"`, `"None"`).
  - `stop_bits` (Integer): Anzahl der Stop-Bits (`1` oder `2`).

---

### `enable [AMPERES] [--power <WATTS>]`
Erteilt die Ladefreigabe. Schreibt das entsprechende Stromlimit in Register `0x0014`.

- **Optionen:**
  - `[AMPERES]`: Stromstärke pro Phase (Standard: `16`).
  - `--power <WATTS>`: Dreiphasige Gesamtleistung (z. B. `11000`), die in Ampere umgerechnet wird.

---

### `disable [--force]`
Entzieht der Wallbox die Ladefreigabe (schreibt `0` in Register `0x0014`), wodurch das Schütz abfällt und der Ladevorgang sofort beendet wird.

- **Optionen:**
  - `-f, --force`: Unterdrückt die Sicherheitsabfrage.

---

### `wake-up`
Führt die genormte Control-Pilot-Sequenz zum Aufwecken schlafender Fahrzeuge aus:
1. Setzt Register `0x0005` auf `0xE0E0` (Zustand E0: CP-Spannung kurz auf 0 V).
2. Wartet 3 Sekunden.
3. Setzt Register `0x0005` auf `0xA1A1` (Zustand A1: CP-Spannung zurück auf +12 V).

---

### `reset [--force]`
Sendet den Befehl `0x5A5A` an Register `0x0005`, um den Mikrocontroller der EVCC2-Steuerung neu zu starten.

- **Optionen:**
  - `-f, --force`: Unterdrückt die Warnabfrage.

---

### `read-all`
Liest Firmware, System-Flags und alle Messwerte in einem einzigen Aufruf aus.

- **JSON-Ausgabe:**
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
Führt eine kontinuierliche Abfrage im konfigurierten Intervall durch (z. B. alle 2 Sekunden).

- **Console-Modus:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon console
  ```
- **MQTT-Modus:**
  ```bash
  abl-evcc tcp 192.168.3.128:8899 daemon mqtt --mqtt-config mqtt.yaml
  ```
- **Veröffentlichte MQTT-Topics:**
  - `<topic>/status`: Zeichen (`A`, `B`, `C`, `D`, `E`, `F`)
  - `<topic>/status_code`: Hex-Code (`0x00A1`, `0x00B1`, `0x00C2`, etc.)
  - `<topic>/status_description`: Textbeschreibung des Zustands
  - `<topic>/ev_connected`: Boolean (`true`/`false`)
  - `<topic>/configured_current`: Stromvorgabe in Ampere pro Phase
  - `<topic>/enabled`: Boolean (`true`, wenn Strom $\ge 6\,\text{A}$)
  - `<topic>/current_l1`, `current_l2`, `current_l3`: Phasenströme in Ampere
  - `<topic>/power`: Gesamt-Wirkleistung in Watt
  - `<topic>/JSON`: Vollständiges JSON-Messwertobjekt

- **MQTT-Fernsteuerung (wenn `allow_control: true` in `mqtt.yaml`):**
  - `<topic>/set/current`: Stromeinstellung in **Ampere pro Phase** (z. B. `16` oder `0`). Gedeckelt durch `max_current`.
  - `<topic>/set/power`: Leistungseinstellung in **Watt für 3 Phasen** (z. B. `11000` oder `0`). Gedeckelt durch `max_current`.
  - `<topic>/set/enable`: `"true"` (aktiviert das Laden mit `max_current` pro Phase) oder `"false"` (Ladestopp).
  - `<topic>/set/wake_up`: `"true"` (führt Wake-Up-Sequenz E0 -> A1 aus).
  - `<topic>/set/reset`: `"true"` (startet den Controller neu).
