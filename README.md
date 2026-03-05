<div align="center">

<p>
<img src="mobile/ios/screenshots/screenshot1.PNG" alt="Dashboard" width="250">&nbsp;&nbsp;<img src="mobile/ios/screenshots/screenshot3.png" alt="Signal Monitor" width="250">&nbsp;&nbsp;<img src="mobile/ios/screenshots/screenshot2.PNG" alt="Router Settings" width="250">
</p>

# ZTE U60 Pro Toolkit

**Unlock the full potential of your ZTE U60 Pro (MU5250) 5G mobile router.**

CLI tools + native mobile companion apps for signal monitoring, band locking,
config backup/decryption, SSH access, network customization, and more.

[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/device-ZTE%20U60%20Pro%20(MU5250)-orange.svg)]()
[![Qualcomm](https://img.shields.io/badge/chipset-Snapdragon%20X75-red.svg)]()
[![OS](https://img.shields.io/badge/firmware-ZWRT%20(OpenWrt%2023.05)-green.svg)]()

</div>

---

## Device

| | |
|---|---|
| **Model** | ZTE U60 Pro (MU5250) |
| **Hardware** | `MU5250_HW1.0` |
| **Firmware** | `CN_ZTE_MU5250V1.0.0B27` (Oct 31, 2025) |
| **Chipset** | Qualcomm Snapdragon X75 (SDX75 / SDXPINN) |
| **CPU** | 4x Cortex-A55 (ARMv8.2-A) @ 2.2 GHz |
| **RAM** | 1.6 GB |
| **Storage** | 8 GB eMMC (Longsys JS08AC), 69 partitions, A/B slots |
| **Modem** | 5G-A Sub-6 + mmWave, Cat 22 LTE |
| **NR Bands** | n1/2/3/5/7/8/18/20/26/28/29/38/40/41/48/66/71/75/77/78/79 |
| **LTE Bands** | 1/2/3/4/5/7/8/18/19/20/26/28/29/32/34/38/39/40/41/42/43/48/66/71 |
| **WiFi** | WiFi 7 (802.11be), 2x2 MIMO, EHT160 |
| **WiFi Chipset** | Qualcomm WCN7851 (`qcacld32`) |
| **WiFi Radios** | 2.4 GHz (ch 1-13, EHT40, 19 dBm) · 5 GHz (ch 36-165, EHT160, 18 dBm) |
| **Battery** | 10,000 mAh Li-ion, 4.5V max, PM7550B fuel gauge |
| **Charging** | USB-PD, 15W (5V/3A), fast charge |
| **USB** | USB-C (PD sink, OTG/powerbank) |
| **Display** | 3.5" IPS LCD (Sitronix ST77926), 320x480, RGB565, DRM/KMS |
| **UI Toolkit** | LVGL with FreeType + LodePNG, assets at `/usr/ui/` |
| **Backlight** | AWINIC AW9523B (I2C `1-005b`), sysfs `/sys/class/leds/led:lcd/brightness` (0-255) |
| **Touch** | Sitronix (I2C `1-0055`) -> `/dev/input/event3` |
| **OS** | ZWRT (OpenWrt 23.05.4 r24012-d8dd03c46f) |
| **Kernel** | Linux 5.15.170-perf, SMP PREEMPT, aarch64 |
| **PMICs** | PMX75 + PM7550BA + PMG1110 |
| **SIM** | Single nano-SIM (no eSIM) |
| **NFC** | Quick device pairing |
| **Clients** | Up to 64 (32 per radio) |

## What's Included

### CLI (`zte` binary)

A single Rust binary with subcommands for full device control over HTTP, ADB, and SSH.

```bash
cargo install --path zte-cli
```

| Command | Description | Interface |
|---|---|---|
| `zte acl` | Manage ubus HTTP ACL (unlock restricted API methods) | Shell |
| `zte companion` | Deploy/manage rpcd Lua plugin for extended API | Shell |
| `zte setup` | All-in-one: enable ADB, install SSH, push keys, patch ACL, install companion | WiFi + ADB |
| `zte monitor` | Live 5G/LTE signal dashboard (ratatui TUI) | Shell |
| `zte network` | DNS, TTL masking, band locking, firewall, telemetry | Shell |
| `zte backup` | Config backup, decrypt, XML viewer, re-encrypt, restore | Shell (local ops too) |
| `zte settings` | 100+ ubus endpoints organized by 12 categories | Shell |
| `zte explore` | Full device system info report | Shell |
| `zte adb-enable` | Enable USB debug mode via WiFi API | WiFi |
| `zte ssh` | Install and start dropbear SSH server | ADB |
| `zte probe` | Enumerate and test ubus HTTP API endpoints | WiFi |

> **Shell** = runs via HTTP (default), `--adb`, or `--ssh` transport. Most commands auto-detect or let you choose.

### Mobile Companion Apps

Native apps that connect directly over WiFi -- no computer needed.

| | iOS | Android |
|---|---|---|
| **Framework** | SwiftUI | Jetpack Compose |
| **Min Version** | iOS 16.0 | Android 8.0 (API 26) |
| **Dependencies** | None (Apple frameworks only) | OkHttp, Hilt, Vico, kotlinx.serialization |
| **Features** | BandLock, Call, Clients, Config, Dashboard, DeviceInfo, Login, RouterSettings, Signal, SIM/STK/USSD, SMS, Tools, USBMode | BandLock, Clients, Config, Dashboard, DeviceInfo, Login, Settings, Signal, Tools |
| **Tabs** | Dashboard, SMS, Tools, Router, Settings | Dashboard, Signal, Settings |
| **Path** | `mobile/ios/ZTECompanion/` (96 Swift files) | `mobile/android/ZTECompanion/` (32 Kotlin files) |

## Device Impact

> **Warning** -- Several commands modify your device's filesystem and firmware settings.
> All write operations require `--confirm` (or are gated behind the `zte setup` wizard).

### Command Impact

| Command | What It Does | Survives Reboot | Undo |
|---|---|---|---|
| `zte setup` | Enables ADB + installs SSH + pushes keys + patches ACL + installs companion (full provisioning) | Yes | See `ssh`, `adb-enable`, `acl`, `companion` below |
| `zte ssh` | Pushes dropbear binary, generates host key, writes `authorized_keys`, hooks `rc.local` | Yes (rc.local hook) | Remove files manually (see Recovery) |
| `zte adb-enable` | Calls `zwrt_bsp.usb.set {mode: "debug"}` to enable USB debug | Yes (firmware setting) | `zwrt_bsp.usb.set {mode: "normal"}` via ubus |
| `zte acl patch` | Bind-mounts modified ACL over `/usr/share/rpcd/acl.d/`, hooks `rc.local` | Yes (rc.local hook) | `zte acl reset` |
| `zte companion install` | Installs rpcd Lua plugin, bind-mounts over `/usr/libexec/rpcd/`, hooks `rc.local` | Yes (rc.local hook) | `zte companion remove` |
| `zte network band --lock` | Writes NR/LTE band lock via firmware API | Yes (firmware setting) | `zte network band --unlock-all --confirm` |
| `zte network ttl --set` | Adds iptables mangle rules for TTL/HL masking | **No** | `zte network ttl --clear --confirm` or reboot |
| `zte network telemetry --disable` | Appends to `/etc/hosts` + adds iptables OUTPUT DROP rules | Partial (/etc/hosts yes, iptables no) | Edit `/etc/hosts` manually; iptables rules clear on reboot |
| `zte network dns --set` | Writes DNS config via UCI | Yes | `zte network dns --set` with original values |
| `zte backup restore` | Overwrites `/userconfig/config.bin` with re-encrypted config | Yes | Only reversible with a prior `zte backup backup` |
| `zte settings display brightness` | Sets UCI brightness + sysfs backlight | Yes (UCI) | `zte settings display brightness <original> --confirm` |
| `zte settings device charge-limit-set` | Sets battery charge policy mode (0-5) | Yes (firmware) | `zte settings device charge-limit-set 0 --confirm` |
| `zte settings device wall-mode` | Toggles direct power supply bypass | Yes | Toggle again |
| `zte settings device fast-boot-on/off` | Toggle suspend-to-RAM | Yes | Toggle again |
| `zte settings schedule reboot` | Sets scheduled auto-reboot | Yes (UCI) | Disable via same command |
| `zte settings device factory-reset` | Full factory reset via `zwrt_bsp.power.factory_reset` | **Irreversible** | N/A -- wipes all data and custom config |

**Read-only commands** (no device changes): `zte monitor`, `zte explore`, `zte probe`, `zte acl show`, `zte companion status`, `zte backup backup`, `zte backup decrypt`, `zte backup view`, `zte settings ... --show`, `zte settings display info`, `zte settings device battery`, `zte settings device charge-limit`, `zte settings device fast-boot`, `zte settings cell stc-status`, `zte settings cell dsds-status`, `zte settings cell signal-detect status`, `zte settings cell sleep-status`, `zte settings cell ps-timer-status`, `zte settings wifi diagnose`, `zte network ... --status/--show/--scan`.

### Filesystem Layout

Files land in two locations on the device:

```
/data/local/tmp/                    (writable /data partition)
├── dropbear                        <- SSH binary (zte ssh)
├── start_ssh.sh                    <- SSH boot script (zte ssh)
├── zte-companion-plugin            <- rpcd Lua plugin (zte companion install)
├── companion_plugin.sh             <- companion boot script
├── rpcd-plugins/                   <- companion bind-mount overlay
├── rpcd-acl.d/                     <- ACL bind-mount overlay (zte acl patch)
└── acl_patch.sh                    <- ACL boot script

/etc/                               (read-only rootfs -- may fail on some firmware)
├── rc.local                        <- boot hooks appended here
└── dropbear/
    ├── authorized_keys             <- your SSH public key
    └── dropbear_rsa_host_key       <- generated host key

/usr/share/rpcd/acl.d/web.json      <- original ACL (bind-mounted over by acl patch)
/usr/libexec/rpcd/                   <- original rpcd plugins (bind-mounted over by companion)
/userconfig/config.bin               <- device config (overwritten by backup restore)
```

### Recovery

- **Factory reset** (`zte settings device factory-reset --confirm`) wipes everything -- all custom files, SSH keys, ACL patches, and companion plugins.
- **Per-feature undo commands:**
  - ACL: `zte acl reset`
  - Companion: `zte companion remove`
  - Bands: `zte network band --unlock-all --confirm`
  - TTL: `zte network ttl --clear --confirm` (or just reboot)
- **Manual SSH removal** (if `zte ssh` was used):
  ```bash
  # Via ADB shell or existing SSH session
  rm /data/local/tmp/dropbear /data/local/tmp/start_ssh.sh
  rm /etc/dropbear/authorized_keys /etc/dropbear/dropbear_rsa_host_key
  sed -i '\|start_ssh.sh|d' /etc/rc.local
  killall dropbear
  ```

## Quick Start

### Prerequisites

```bash
# 1. Install Rust (if you don't have it) -- see https://rustup.rs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Install ADB (for USB-based tools)
brew install android-platform-tools   # macOS
sudo apt install adb                  # Linux

# 3. Build the CLI
cargo build --release
# Binary at: ./target/release/zte

# 4. (Optional) Install globally
cargo install --path zte-cli
```

### All-in-One Setup

The fastest path from unboxing to full control. One command handles everything — also the only command needed after a factory reset:

```bash
# Enables ADB, installs SSH, patches ACL, installs companion — one command
zte setup --password YOUR_PASSWORD

# Custom SSH port and key
zte setup --password YOUR_PASSWORD --port 2222 --key ~/.ssh/id_ed25519.pub
```

### Enable ADB

If you prefer step-by-step over `zte setup`:

```bash
# Over WiFi (no ADB needed)
zte adb-enable --gateway 192.168.0.1 --password YOUR_PASSWORD
```

### Signal Monitor

```bash
# Via HTTP (default, needs password)
zte monitor --password YOUR_PASSWORD

# Via ADB (local ubus, no auth needed)
zte monitor --adb

# Via SSH
zte monitor --ssh --port 2222
```

### Network Tools

```bash
# Band locking
zte network band --status
zte network band --lock 77,78 --confirm
zte network band --lock-lte 1,3,7 --confirm
zte network band --unlock-all --confirm

# DNS
zte network dns --show
zte network dns --set 1.1.1.1 8.8.8.8 --confirm

# TTL masking (bypass tethering detection)
zte network ttl --set 65 --confirm
zte network ttl --status

# Telemetry blocking
zte network telemetry --scan
zte network telemetry --disable --confirm

# Firewall
zte network firewall --show
```

### Config Backup & Decrypt

```bash
# Backup device config
zte backup backup ./backups/

# Decrypt a config.bin
zte backup decrypt config.bin -o config.xml

# View config as XML tree
zte backup view config.bin

# Re-encrypt and restore to device
zte backup restore config.xml --confirm
```

### Enable SSH

```bash
zte ssh --port 2222 --key ~/.ssh/id_ed25519.pub --confirm
# Then: ssh root@192.168.0.1 -p 2222
```

### HTTP ACL

```bash
# Show current ubus HTTP ACL
zte acl show

# Unlock restricted objects (luci-rpc, network.*)
zte acl patch

# Reset ACL to factory default
zte acl reset
```

### HTTP API Probe

```bash
# Full probe with authentication
zte probe --password YOUR_PASSWORD

# Anonymous-only, skip actual method calls
zte probe --skip-calls

# Include write methods (caution!)
zte probe --password YOUR_PASSWORD --include-writes

# Custom output and delay
zte probe --password YOUR_PASSWORD --output report.json --delay 0.2 --verbose
```

### Advanced Settings

```bash
# 100+ ubus endpoints organized by 12 categories
zte settings --help

# Categories: network, cell, apn, wifi, dns, firewall, qos, vpn, lan, device, display, schedule

# Examples:
zte settings network mode --show
zte settings wifi status
zte settings apn list
zte settings display info
zte settings device battery
zte settings device charge-limit
zte settings cell stc-status
zte settings wifi diagnose    # macOS only
```

## Architecture

### API Layer

The ZTE U60 Pro exposes a **ubus JSON-RPC 2.0** API at `http://<gateway>/ubus/`.

```
POST http://192.168.0.1/ubus/?t=1709142000000
Content-Type: application/json

[{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "call",
  "params": ["<session>", "zte_nwinfo_api", "nwinfo_get_netinfo", {}]
}]
```

**Authentication** uses a salt-based double-SHA256 challenge:

```
1. GET salt:    zwrt_web.web_login_info  ->  { "zte_web_sault": "..." }
2. Hash:        UPPER(SHA256(UPPER(SHA256(password)) + salt))
3. Login:       zwrt_web.web_login { "password": hash }  ->  session token
```

> Note: The salt field is `zte_web_sault` (ZTE typo). Fetch can be flaky -- retry up to 3 times.

### ADB Shell (local ubus)

When connected via USB, `adb shell ubus call` works **without authentication** -- preferred for local tools:

```bash
adb shell ubus call zte_nwinfo_api nwinfo_get_netinfo '{}'
adb shell ubus call zwrt_bsp.battery list '{}'
adb shell ubus call zwrt_bsp.thermal get_cpu_temp '{}'
```

### Key ubus Endpoints

<details>
<summary>Signal & Network</summary>

| Endpoint | Description |
|---|---|
| `zte_nwinfo_api.nwinfo_get_netinfo` | NR/LTE/WCDMA signal, band, operator |
| `zte_nwinfo_api.nwinfo_set_nrbandlock` | Lock NR5G bands (NSA/SA) |
| `zte_nwinfo_api.nwinfo_set_gwl_bandlock` | Lock LTE bands |
| `zte_nwinfo_api.nwinfo_rest_band_rat` | Unlock all bands |
| `network.device.status` | Interface traffic stats (`rmnet_data0`) |
| `network.interface.zte_wan.status` | WAN IPv4 address |
| `network.interface.zte_wan6.status` | WAN IPv6 address |
| `network.interface.lan.status` | LAN/gateway IP |

</details>

<details>
<summary>Device & Hardware</summary>

| Endpoint | Description |
|---|---|
| `zwrt_bsp.battery.list` | Battery %, temperature |
| `zwrt_bsp.thermal.get_cpu_temp` | CPU temperature |
| `zwrt_bsp.usb.set` | USB mode (debug/normal) |
| `zwrt_bsp.charger.set` | Wall mode (direct power supply bypass) |
| `zwrt_bsp.charger.list` | Charge policy, battery current/voltage |
| `zwrt_zte_mdm.api.get_sim_info` | ICCID, IMSI, MSISDN |
| `zwrt_zte_mdm.api.get_imei` | Device IMEI |

</details>

<details>
<summary>Display</summary>

| Endpoint / Path | Description |
|---|---|
| `zwrt_deviceui.Screen` (UCI) | Brightness (0-5), timeout, wakeup, PIN lock |
| `zwrt_led.nightmode` (UCI) | Night mode switch, start/end time |
| `/sys/class/leds/led:lcd/brightness` | Backlight sysfs (0-255) |
| `/sys/kernel/debug/qpic_display/is_panel_on` | Panel power state |
| `/sys/kernel/debug/qpic_display/draw` | Debug draw interface (write-only) |

</details>

<details>
<summary>SIM / STK</summary>

| Endpoint | Description |
|---|---|
| `zwrt_zte_mdm.api.get_sim_lock_info` | PIN/PUK status, attempt counts |
| `zwrt_zte_mdm.api.verify_pin` | Unlock SIM with PIN |
| `zwrt_zte_mdm.api.verify_puk` | Unlock SIM with PUK |
| `zte-companion.stk_get_menu` | SIM Toolkit menu items |
| `zte-companion.ussd_send` | Send USSD code (*100#, etc.) |

</details>

<details>
<summary>Connected Devices</summary>

| Endpoint | Description |
|---|---|
| `luci-rpc.getHostHints` | MAC, hostname, IPs of clients |
| `luci-rpc.getDHCPLeases` | DHCP lease enrichment |
| `zwrt_wlan.status` | WiFi SSIDs, encryption, power |
| `zwrt_wlan.get_assoc_info` | Connected client details |

</details>

<details>
<summary>Authentication</summary>

| Endpoint | Description |
|---|---|
| `zwrt_web.web_login_info` | Fetch login salt (anonymous) |
| `zwrt_web.web_login` | Authenticate with hashed password |

</details>

### Config Encryption

ZTE config backups (`.bin`) use a custom format:

```
+---------------------------------+
|  Header (128 bytes)             |
|  +-- Magic: "ZXHN"  [0x00-03]  |
|  +-- Type:  ECB/CBC  [0x04]    |
|  +-- Signature       [0x08-47] |
|  +-- Payload offset  [0x48-4B] |
+---------------------------------+
|  Payload (encrypted+compressed) |
|  +-- AES-128-ECB (type 0)      |
|  +-- AES-256-CBC (type 1, 3)   |
|  |   +-- First 16 bytes = IV   |
|  +-- Plain (type 2)            |
|  +-- ZLIB compressed           |
|  |   +-- Standard zlib         |
|  |   +-- Chunked (4B BE+zlib)  |
|  |   +-- Raw deflate           |
+---------------------------------+
```

Key resolution: 14 known static keys + `MD5(serial)[:16]` + `MD5(signature)[:16]`.

## Project Structure

```
u60-Pro-rs/
├── zte-cli/                   CLI binary (zte)
│   └── src/
│       ├── main.rs            Entry point + clap
│       ├── cmd/               Subcommands
│       │   ├── acl.rs         Manage ubus HTTP ACL
│       │   ├── adb_enable.rs  Enable USB debug via WiFi API
│       │   ├── backup.rs      Config backup/decrypt/view/restore
│       │   ├── companion.rs   Deploy/manage rpcd Lua plugin
│       │   ├── explore.rs     Device info collector
│       │   ├── monitor.rs     Live signal dashboard (ratatui TUI)
│       │   ├── network.rs     DNS, TTL, band lock, firewall, telemetry
│       │   ├── probe.rs       HTTP API endpoint prober
│       │   ├── setup.rs       All-in-one ADB + SSH + keys setup
│       │   ├── settings/      100+ ubus settings endpoints
│       │   │   ├── network.rs, cell.rs, apn.rs, wifi.rs, dns.rs
│       │   │   ├── firewall.rs, qos.rs, vpn.rs, lan.rs
│       │   │   └── device.rs, display.rs, schedule.rs
│       │   └── ssh.rs         Install dropbear SSH via ADB
│       └── ui/                TUI rendering
│           ├── colors.rs      Signal-quality color mapping
│           ├── csv_logger.rs  CSV export for signal data
│           └── panels.rs      Dashboard panel layout
│
├── zte-lib/                   Shared library
│   └── src/
│       ├── ubus.rs            HTTP JSON-RPC 2.0 + auth
│       ├── adb.rs             ADB wrapper
│       ├── ssh.rs             SSH/SCP transport
│       ├── device.rs          Unified device shell (ADB/SSH/HTTP)
│       ├── transport.rs       Transport abstraction
│       ├── at.rs              AT serial interface
│       ├── error.rs           Error types
│       ├── signal/            Signal monitoring
│       │   ├── collector.rs   Periodic signal data collection
│       │   ├── parsers.rs     NR/LTE signal response parsing
│       │   └── types.rs       Signal metric types + thresholds
│       └── zcu/               ZTE Config Utility
│           ├── config.rs      Header parse, decrypt/encrypt pipeline
│           ├── crypto.rs      AES-128-ECB, AES-256-CBC
│           ├── compression.rs ZLIB plain + chunked
│           ├── keys.rs        14 static keys + derivation
│           └── constants.rs   Magic numbers, offsets
│
└── mobile/                    Native companion apps
    ├── README.md              Mobile apps documentation
    ├── ios/ZTECompanion/      SwiftUI app (96 Swift files)
    │   ├── Core/              Networking, crypto, models
    │   ├── Features/
    │   │   ├── BandLock/      Band locking
    │   │   ├── Call/          Call forwarding
    │   │   ├── Clients/       Connected devices
    │   │   ├── Config/        Config backup/restore
    │   │   ├── Dashboard/     Signal cards, WiFi card, status
    │   │   ├── DeviceInfo/    Hardware + firmware info
    │   │   ├── Login/         Auth + keychain
    │   │   ├── RouterSettings/
    │   │   │   ├── APN/       APN profile management
    │   │   │   ├── CellLock/  Cell locking
    │   │   │   ├── Device/    Reboot, USB, battery, charge policy
    │   │   │   ├── DNS/       DNS settings
    │   │   │   ├── Firewall/  Firewall rules
    │   │   │   ├── LAN/       DHCP settings
    │   │   │   ├── MobileNetwork/  Network mode, SA/NSA
    │   │   │   ├── NetworkMode/    Network selection
    │   │   │   ├── QoS/       Traffic shaping
    │   │   │   ├── Schedule/  Scheduled reboot/wifi
    │   │   │   ├── SignalDetect/   Signal quality detection
    │   │   │   ├── SIM/       SIM card PIN/PUK management
    │   │   │   │   └── STK/   SIM Toolkit & USSD
    │   │   │   ├── STC/       Smart Tower Connect
    │   │   │   ├── Telemetry/ Telemetry blocking
    │   │   │   ├── VPN/       VPN settings
    │   │   │   └── WiFi/      WiFi configuration
    │   │   ├── Settings/      App settings
    │   │   ├── Signal/        Signal monitor
    │   │   ├── SMS/           SMS compose/read/conversations
    │   │   ├── Tools/         Band lock, clients, config, etc.
    │   │   └── USBMode/       USB mode detection/switching
    │   └── Navigation/        5-tab bar (Dashboard, SMS, Tools, Router, Settings)
    └── android/ZTECompanion/  Jetpack Compose app (32 Kotlin files)
        ├── core/              Network, crypto, models, DI
        ├── feature/           bandlock, clients, config, dashboard,
        │                      deviceinfo, login, settings, signal, tools
        └── navigation/        NavHost + bottom bar
```

## Signal Thresholds

| Metric | Excellent | Good | Fair | Poor |
|---|---|---|---|---|
| **RSRP** (dBm) | >= -80 | >= -100 | >= -110 | < -110 |
| **SINR** (dB) | >= 20 | >= 10 | >= 0 | < 0 |
| **RSRQ** (dB) | >= -10 | >= -15 | >= -20 | < -20 |

## ZTE Telemetry Domains

The following domains are blocked by `zte network telemetry --disable`:

```
iot.zte.com.cn          mifi.zte.com.cn         update.zte.com.cn
cpe.zte.com.cn          fota.zte.com.cn         push.zte.com.cn
log.zte.com.cn          report.zte.com.cn       cloud.zte.com.cn
ztedevices.com          www.ztedevices.com       support.ztedevices.com
```

## Technical Notes

- Device ID reports as `MU5120ZTED0000000` (not MU5250)
- Anonymous session token: `00000000000000000000000000000000` (32 zeros)
- AT serial device: `/dev/at_mdm0` (requires background-cat read method)
- No dropbear/sshd pre-installed; `zte ssh` deploys it
- Local `adb shell ubus call` works without HTTP auth
- Session tokens expire after ~5 minutes; tools auto-retry
- Salt fetch field name: `zte_web_sault` (ZTE typo, not `salt`)
- WiFi 5GHz radio: Qualcomm WCN7851, 21 dBm (~126 mW) at 40% power, EHT160 (WiFi 7); `txpowerpercent` adjustable 10-100% in UCI
- **CPU**: OpenWrt target says `aarch64_cortex-a53` but CPU part `0xd05` variant `0x2` = Cortex-A55 (ARMv8.2-A). Frequencies: 691 MHz - 2.2 GHz
- **Board**: Device-tree model `Qualcomm Technologies, Inc. SDXPINN IDP MBB`, board `qcom,sdxpinn-idp`
- **eMMC**: Longsys `JS08AC`, manufacturer ID `0x0000f2`, 8 GB (7,389,184 blocks)
- **Boot**: A/B partition scheme (`SLOT_SUFFIX=_a`), SELinux enforcing
- **Battery model**: `7527761_ZTE_MU5250_HIGHPOWER_10000MAH_PM7550B` (design capacity 10,214 uAh)
- **Thermal**: 40 thermal zones -- CPU (cpuss-0..3), modem (mdmss-0..2, mdmq6-0), mmWave (mmw0..3), PMICs (pmx75, pm7550ba, pmg1110), USB, battery, ethphy
- **AI partition**: `/ai_app` (365 MB) contains `xDpi_SigLibSoft.bin` (signal processing model)
- **WiFi driver**: `qcacld32` (Qualcomm Connected Audio/Lighting/Data 3.2)
- **Firmware build**: `BD_CNMU5250V1.0.0B27`, integrated `CN_ZTE_MU5250V1.0.0B27`, build date Oct 31 2025
- **Display panel**: Sitronix ST77926 IPS LCD, 320x480 pixels, RGB565 (16-bit color)
- **Display rendering**: No framebuffer (`/dev/fb0` absent) -- DRM/KMS only (`/dev/dri/card0`); LVGL UI toolkit with FreeType + LodePNG
- **Display assets**: PNG skins/animations/icons/fonts at `/usr/ui/`; UI daemon `zte_topsw_devui`, LED daemon `zte_topsw_led`
- **Display debug**: `/sys/kernel/debug/qpic_display/draw` (write-only), `image_dump`; daemon can snapshot to `/cache/fb.png`
- **Backlight controller**: AWINIC AW9523B at I2C `1-005b`; touch controller Sitronix at I2C `1-0055` -> `/dev/input/event3`
- **Charge policy**: sysfs `ui_chg_policy_mode` (0-5), targets SOC range; mode 5 = 80-100%
- **Wall mode**: `zwrt_bsp.charger set direct_power_supply_mode` -- direct power bypass, battery not used

## License & Disclaimers

This project is licensed under the [MIT License](LICENSE).

**Disclaimer:** This software is provided "as is", without warranty of any kind. Use it at your own risk. The authors are not responsible for any damage to your device, voided warranties, or other consequences arising from the use of this software.

- **Not affiliated with ZTE Corporation.** This is an independent community project.
- Intended for use on devices you personally own.
- Reverse engineering was performed solely for interoperability and educational purposes.
- Encryption keys and protocol details are sourced from publicly available community research.
- No proprietary source code from ZTE is included in this repository.
