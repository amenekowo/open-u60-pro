<div align="center">

<p>
<img src="mobile/ios/screenshots/screenshot1.PNG" alt="Dashboard" width="250">&nbsp;&nbsp;<img src="mobile/ios/screenshots/screenshot3.png" alt="Signal Monitor" width="250">&nbsp;&nbsp;<img src="mobile/ios/screenshots/screenshot2.PNG" alt="Router Settings" width="250">
</p>

# ZTE U60 Pro Toolkit

**Unlock the full potential of your ZTE U60 Pro (MU5250) 5G mobile router.**

On-device REST agent + native mobile companion apps for signal monitoring, band locking,
config backup/decryption, network customization, and more.

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
| **Bluetooth** | Qualcomm WCN7850 (BT 5.3+), UART transport — disabled (services stopped, module unloaded) |
| **Clients** | Up to 64 (32 per radio) |

## What's Included

### On-Device Agent (`zte-agent`)

A lightweight Rust HTTP server that runs directly on the router (port 9090, LAN-only). It proxies ubus calls, AT commands, and sysfs reads into a typed REST API that mobile apps consume over WiFi.

```bash
# Cross-compile for the device
cargo build --release --target aarch64-unknown-linux-musl -p zte-agent
```

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

> **Warning** -- The agent exposes device control endpoints that modify firmware settings.
> Destructive actions (factory reset, reboot) require confirmation in the mobile app.

### Filesystem Layout

```
/data/local/tmp/                    (writable /data partition)
├── zte-agent                       <- on-device REST agent
├── start_zte_agent.sh              <- agent boot script
├── dropbear                        <- SSH binary (if installed)
└── start_ssh.sh                    <- SSH boot script (if installed)

/etc/                               (read-only rootfs)
├── rc.local                        <- boot hooks appended here
└── dropbear/
    ├── authorized_keys             <- SSH public key (if installed)
    └── dropbear_rsa_host_key       <- generated host key (if installed)
```

## Quick Start

### Prerequisites

```bash
# 1. Install Rust -- see https://rustup.rs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Add musl cross-compilation target
rustup target add aarch64-unknown-linux-musl

# 3. Build the agent
cargo build --release --target aarch64-unknown-linux-musl -p zte-agent
```

### Deploy Agent

Push the binary to the device via ADB or SCP, then start it:

```bash
# Via ADB
adb push target/aarch64-unknown-linux-musl/release/zte-agent /data/local/tmp/
adb shell "ZTE_AGENT_PASSWORD=yourpassword /data/local/tmp/zte-agent &"

# Via SCP (if SSH is installed)
scp -P 2222 target/aarch64-unknown-linux-musl/release/zte-agent root@192.168.0.1:/data/local/tmp/
ssh -p 2222 root@192.168.0.1 "ZTE_AGENT_PASSWORD=yourpassword /data/local/tmp/zte-agent &"
```

### Connect Mobile App

1. Connect your phone to the router's WiFi
2. Open ZTE Companion app
3. Set agent URL: `http://192.168.0.1:9090`
4. Enter the agent password you set above

## Project Structure

```
u60-Pro-rs/
├── zte-agent/                 On-device REST API server
│   └── src/
│       ├── main.rs            Entry point (Axum HTTP server)
│       └── routes/            REST endpoint handlers
│
└── mobile/                    Native companion apps
    ├── README.md              Mobile apps documentation
    ├── ios/ZTECompanion/      SwiftUI app
    │   ├── Core/              Networking, crypto, models
    │   ├── Features/
    │   │   ├── BandLock/      Band locking
    │   │   ├── Call/          Voice calls
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
    └── android/ZTECompanion/  Jetpack Compose app
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

The following domains are blocked by the telemetry blocker:

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
- No dropbear/sshd pre-installed; can be deployed manually via ADB
- Session tokens expire after ~5 minutes; the agent auto-retries
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
