# ABL EVCC1 Modbus & Schnittstellendokumentation

Dieses Dokument beschreibt die Register und Schnittstellen für ABL EVCC1 Ladecontroller (u.a. verbaut in frühen ABL eMH1 Wallboxen) und stellt die historische Entwicklung sowie die Nachweise aus Dokumentationen und Open-Source-Implementierungen (insbesondere `evcc-io/evcc`) dar.

---

## 1. Übersicht der Register bei EVCC1 (Modbus Holding Register)

| Register (Dezimal) | Register (Hex) | Typ / Richtung | Funktion / Beschreibung bei EVCC1 |
|:---:|:---:|:---:|:---|
| **256** | `0x0100` | Read (Holding) | **Ladestatus (Fahrzeug-Status):** Liefert numerische ASCII-Werte des Control-Pilot-Status nach IEC 61851-1 zurück.<br>Typischerweise:<br>• **65** (`0x41` / `'A'`): Standby / Bereit (kein Fahrzeug verbunden)<br>• **66** (`0x42` / `'B'`): Fahrzeug verbunden, wartet auf Ladefreigabe<br>• **67** (`0x43` / `'C'`): Ladevorgang aktiv (Fahrzeug lädt)<br>• **68** (`0x44` / `'D'`): Laden mit externer Belüftung<br>• Weitere ASCII-Werte (`'E'` für Fehler/abgeschaltet, `'F'` für Fehlerfall etc.) |
| **257** | `0x0101` | Read / Write | **Maximaler Ladestrom (Vorgabe & Status):**<br>• Dient gleichzeitig als Statusabfrage für die Freigabe und zum Einstellen der Stromstärke.<br>• Wert in ganzzahligen Ampere (z. B. `16` = 16 Ampere, `0` = Deaktiviert / Freigabe entzogen, Wertebereich 6 bis 80 Ampere bzw. 0 für Halt). |
| **258** | `0x0102` | Read (Holding) | **Aktuelle Ladeleistung / Stromstärke:**<br>• Je nach Firmware-Revision der eMH1 Master liegen hier die Messwerte für Ladeleistung oder Phasenströme vor (bzw. Summenstrom / Momentanleistung). |

---

## 2. Nachweise und Recherche im `evcc-io/evcc` Repository

Im Repository `https://github.com/evcc-io/evcc` existieren zwei wesentliche historische Implementierungsstränge für ABL-Wallboxen:

### A. ABL SURSUM EVCC (Pre-Modbus proprietäres ASCII-Protokoll)
- **Commit:** `cbeed0120e2c24f39337da86748ba94791e2f80f` ("Add ABL SURSUM EVCC controller")
- **Dateien:** `charger/abl-evcc.go`, `charger/ablevcc/connection.go`, `templates/definition/charger/abl-evcc.yaml`
- **Beschreibung:** Für frühe ABL-Wallboxen mit dem ursprünglichen EVCC-Steuermodul von ABL SURSUM (vor Einführung des Modbus-Protokolls). Hierbei sendet der Master Textkommandos der Form `!n cc [data]\r\n` (z. B. `!1 01` für Firmware, `!1 02` für Statusabfrage, `!1 12 0016` für PWM/Ladestrom).
- **Referenzdokument:** Das offizielle ABL SURSUM Benutzerhandbuch *Dok. Nr. 03 0009 01 D* (siehe `docs/EVCC.pdf`).

### B. ABL eMH1 / eMH2 (Modbus-basierte EVCC-Generation)
- **Dateien:** `charger/abl.go`, `templates/definition/charger/abl.yaml`
- **Registerbereich EVCC2/3:** Verwendet Modbus ASCII mit 38400 8E1 und Registern im niedrigen Bereich (`0x0001` Firmware, `0x0004` Status, `0x0014` Amps Config via Tastgrad in 0,1%-Schritten, `0x002E` Strom-Messwerte).
- **EVCC1 Modbus Register 256–258:** In Übergangs-Firmwares und spezifischen eMH1 Master-Modellen existiert die Modbus-Schnittstelle mit linearen Holding-Registern ab `256` (`0x0100`), die direkt ASCII-Zustände (`'A'`, `'B'`, `'C'`) und Ampere-Werte (`257`) abbilden.

---

## 3. Offizielle Dokumente im Verzeichnis `docs/`

1. **`docs/EVCC.pdf`**:
   - Offizielle Montage- und Bedienungsanleitung von ABL SURSUM (*Dok. Nr. 03 0009 01 D*).
   - Enthält die Pinbelegung, Schaltungsempfehlungen und die RS485-Befehle (`!n 01`, `!n 02`, `!n 11`, `!n 12`, etc.) des originalen EVCC-Moduls.
2. **`docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf`**:
   - Offizielle Spezifikation von ABL für die EVCC2/3-Schnittstelle (Modbus ASCII, Register `0x0001` bis `0x0057`).
