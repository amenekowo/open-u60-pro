# ZTE Companion — Mobile Apps for ZTE U60 Pro (MU5250)

Native companion apps for the ZTE U60 Pro 5G mobile router. Connect directly to the router's ubus HTTP API over WiFi — no intermediate server, no ADB required.

## Features

| Feature | Status | Details |
|---|---|---|
| Signal Monitoring | Full | Live NR 5G / LTE / WCDMA metrics with color-coded thresholds |
| RSRP History Chart | Full | Scrollable chart tracking signal strength over time |
| Battery & Thermal | Full | Battery %, temperature, CPU thermal |
| Traffic Stats | Full | Real-time DL/UL speed (Mbps), total bytes transferred |
| Connected Devices | Full | MAC, hostname, IPv4/IPv6 via host hints + DHCP enrichment |
| Device Info | Full | SIM (ICCID, IMSI, MSISDN), IMEI, WAN/LAN IPs |
| Band Lock/Unlock | Full | Lock NR5G NSA/SA and LTE bands, unlock all |
| Enable ADB | Full | One-tap USB debug mode via WiFi |
| Config Decrypt/Encrypt | Full | Import .bin, auto-detect key, browse XML, re-encrypt, export |
| DNS, TTL, Firewall, Telemetry, SSH, Explorer | Placeholder | Requires ADB USB — shown with informational message |

## Architecture

- **Pattern**: MVVM on both platforms
- **Transport**: Direct HTTP to router's ubus JSON-RPC 2.0 API (`http://<gateway>/ubus/`)
- **Auth**: Salt-based double-SHA256 challenge-response with auto-retry on session expiry

## iOS App

**Path**: `ios/ZTECompanion/`

### Requirements

- iOS 16.0+
- Xcode 15+
- No external dependencies (uses only Apple frameworks)

### Tech Stack

| Component | Implementation |
|---|---|
| UI | SwiftUI |
| HTTP | URLSession |
| Auth hashing | CryptoKit (SHA-256) |
| Config crypto | CommonCrypto (AES-128-ECB, AES-256-CBC) |
| Compression | Compression framework (zlib) |
| Charts | Swift Charts |
| Secure storage | Keychain Services |
| Key derivation | CryptoKit (Insecure.MD5) |

### Project Structure

```
ios/ZTECompanion/
├── ZTECompanionApp.swift                  App entry point
├── Core/
│   ├── Networking/
│   │   ├── UbusClient.swift               JSON-RPC 2.0 client (URLSession)
│   │   ├── AuthManager.swift              Salt fetch, double-SHA256, Keychain helper
│   │   └── UbusError.swift                Error types
│   ├── Crypto/
│   │   ├── ZTEConfigCrypto.swift          AES-ECB/CBC, header parsing, key derivation
│   │   └── ZTECompression.swift           ZLIB plain/chunked/raw decompress + compress
│   ├── Models/
│   │   ├── SignalModels.swift             NRSignal, LTESignal, WCDMASignal, OperatorInfo
│   │   ├── DeviceModels.swift             Battery, Thermal, Traffic, ConnectedDevice
│   │   ├── BandModels.swift               BandConfig
│   │   └── ConfigModels.swift             ConfigHeader, PayloadType, known keys table
│   └── Extensions/
│       └── ColorExtensions.swift          RSRP/SINR color thresholds
├── Features/
│   ├── Dashboard/                         Summary cards (signal, battery, speed, devices)
│   ├── Signal/                            Live NR/LTE/WCDMA panels + RSRP chart
│   ├── BandLock/                          NR/LTE band selection grid, lock/unlock
│   ├── DeviceInfo/                        SIM, IMEI, WAN/LAN IPs
│   ├── Clients/                           Connected devices list
│   ├── Config/                            Import, decrypt, XML browser, re-encrypt, export
│   ├── Tools/                             Tools list, Enable ADB, placeholder screens
│   ├── Settings/                          Gateway IP, password, poll interval, theme
│   └── Login/                             Modal login overlay
└── Navigation/
    └── TabBarView.swift                   Dashboard | Signal | Tools | Settings
```

### Setup

1. Open the project in Xcode
2. Add the source files to a new iOS App target (SwiftUI lifecycle)
3. Add `NSAppTransportSecurity` → `NSAllowsArbitraryLoads = YES` to `Info.plist`
4. Build and run on device or simulator

## Android App

**Path**: `android/ZTECompanion/`

### Requirements

- Android 8.0+ (API 26)
- Android Studio Hedgehog (2023.1) or newer
- JDK 17

### Tech Stack

| Component | Implementation |
|---|---|
| UI | Jetpack Compose + Material 3 |
| HTTP | OkHttp 4.12 |
| JSON | kotlinx.serialization 1.7 |
| Auth hashing | java.security.MessageDigest (SHA-256) |
| Config crypto | javax.crypto.Cipher (AES-128-ECB, AES-256-CBC) |
| Compression | java.util.zip.Inflater |
| Charts | Vico 2.0 |
| DI | Hilt 2.54 |
| Secure storage | EncryptedSharedPreferences |
| Navigation | Navigation Compose |

### Project Structure

```
android/ZTECompanion/
├── build.gradle.kts                       Root build config (AGP, Kotlin, Hilt, KSP)
├── settings.gradle.kts                    Module includes
├── gradle.properties                      AndroidX, non-transitive R, JVM args
└── app/
    ├── build.gradle.kts                   Dependencies (Compose, OkHttp, Hilt, Vico, etc.)
    └── src/main/
        ├── AndroidManifest.xml            INTERNET permission, cleartext traffic
        ├── res/
        │   ├── xml/network_security_config.xml
        │   └── values/                    strings.xml, colors.xml, themes.xml
        └── java/com/ztecompanion/
            ├── ZTECompanionApp.kt         @HiltAndroidApp application class
            ├── MainActivity.kt            Single-activity Compose host
            ├── core/
            │   ├── network/
            │   │   ├── UbusClient.kt      JSON-RPC 2.0 client (OkHttp)
            │   │   ├── AuthManager.kt     Auth + EncryptedSharedPreferences + gateway detect
            │   │   └── UbusError.kt       Sealed class error hierarchy
            │   ├── crypto/
            │   │   ├── ZTEConfigCrypto.kt AES-ECB/CBC, header parsing, key derivation
            │   │   └── ZTECompression.kt  ZLIB plain/chunked/raw decompression
            │   ├── model/
            │   │   ├── SignalModels.kt    NRSignal, LTESignal, WCDMASignal, OperatorInfo
            │   │   ├── DeviceModels.kt    Battery, Thermal, Traffic, ConnectedDevice
            │   │   ├── BandModels.kt      BandConfig
            │   │   └── ConfigModels.kt    ConfigHeader, PayloadType, known keys table
            │   └── di/
            │       └── AppModule.kt       Hilt module (UbusClient, AuthManager providers)
            ├── feature/
            │   ├── dashboard/             Summary cards (signal, battery, speed, devices)
            │   ├── signal/                Live NR/LTE/WCDMA panels + RSRP sparkline
            │   ├── bandlock/              NR/LTE band chips, lock/unlock
            │   ├── deviceinfo/            SIM, IMEI, WAN/LAN IPs
            │   ├── clients/               Connected devices list
            │   ├── config/                SAF import, decrypt, XML viewer, re-encrypt, export
            │   ├── tools/                 Tools list, Enable ADB, placeholder screens
            │   ├── settings/              Gateway, poll interval, dark mode, logout
            │   └── login/                 Login form with gateway field
            └── navigation/
                └── AppNavigation.kt       NavHost + BottomNavBar
```

### Setup

1. Open `android/ZTECompanion/` in Android Studio
2. Sync Gradle (dependencies download automatically)
3. Build and run on device or emulator

## How It Works

### Connection

Both apps connect directly to the router over WiFi. The router exposes a ubus JSON-RPC 2.0 API at `http://<gateway>/ubus/`.

**Gateway auto-detection:**
- iOS: `getifaddrs()` to find the default gateway
- Android: `WifiManager.getDhcpInfo().gateway`
- Fallback: `192.168.0.1`

### Authentication

The ZTE U60 Pro uses a salt-based double-SHA256 challenge-response:

1. Fetch salt (anonymous call to `zwrt_web.web_login_info` — field name is `zte_web_sault`)
2. Hash: `UPPER(SHA256(UPPER(SHA256(password)) + salt))`
3. Login: `zwrt_web.web_login` with the hash → receive session token
4. Session tokens expire after ~5 minutes; the apps automatically re-authenticate

Salt fetch can be flaky — both apps retry up to 3 times with 500ms delays.

### ubus API Endpoints

| Feature | Object | Method | Params |
|---|---|---|---|
| Signal data | `zte_nwinfo_api` | `nwinfo_get_netinfo` | `{}` |
| Battery | `zwrt_bsp.battery` | `list` | `{}` |
| CPU temperature | `zwrt_bsp.thermal` | `get_cpu_temp` | `{}` |
| Traffic stats | `network.device` | `status` | `{"name":"rmnet_data0"}` |
| Connected devices | `luci-rpc` | `getHostHints` | `{}` |
| DHCP leases | `luci-rpc` | `getDHCPLeases` | `{"family":4}` |
| SIM info | `zwrt_zte_mdm.api` | `get_sim_info` | `{}` |
| IMEI | `zwrt_zte_mdm.api` | `get_imei` | `{}` |
| WAN IPv4 | `network.interface.zte_wan` | `status` | `{}` |
| WAN IPv6 | `network.interface.zte_wan6` | `status` | `{}` |
| LAN/Gateway | `network.interface.lan` | `status` | `{}` |
| Lock NR bands | `zte_nwinfo_api` | `nwinfo_set_nrbandlock` | `{"nr5g_type":"nsa","nr5g_band":"77,78"}` |
| Lock LTE bands | `zte_nwinfo_api` | `nwinfo_set_gwl_bandlock` | `{"is_lte_band":"1","lte_band_mask":"1,3,7",...}` |
| Unlock all bands | `zte_nwinfo_api` | `nwinfo_rest_band_rat` | `{}` |
| Enable ADB | `zwrt_bsp.usb` | `set` | `{"mode":"debug"}` |
| Fetch auth salt | `zwrt_web` | `web_login_info` | `{}` |
| Login | `zwrt_web` | `web_login` | `{"password":"<hash>"}` |

### Signal Color Thresholds

| Metric | Green | Yellow | Orange | Red |
|---|---|---|---|---|
| RSRP (dBm) | >= -80 | >= -100 | >= -110 | < -110 |
| SINR (dB) | >= 20 | >= 10 | >= 0 | < 0 |

### Config Decrypt/Encrypt

The apps can decrypt and re-encrypt ZTE router configuration backup files (`.bin`):

- **Header**: 128 bytes starting with `ZXHN` magic
  - Payload type at offset 4: ECB (0), CBC (1), Plain (2), CBC New (3)
  - Signature at offset 8 (max 64 bytes, null-terminated)
  - Payload offset at offset 72 (4-byte big-endian)
- **Encryption**: AES-128-ECB (16-byte key) or AES-256-CBC (32-byte key, first 16 bytes of payload = IV)
- **Compression**: ZLIB — plain, chunked (4-byte BE length prefix per chunk), or raw deflate
- **Key resolution**: Tries 14 known static keys + MD5(serial)[:16] + MD5(signature)[:16]

## Navigation

```
Tab Bar
├── Dashboard ─── Summary cards (signal, battery, speed, device count)
├── Signal ────── Live NR/LTE/WCDMA panels + RSRP history chart
│   └── Band Lock (sub-screen)
├── Tools ─────── List of tools:
│   ├── Device Info (SIM, IMEI, IPs)
│   ├── Connected Devices
│   ├── Enable ADB
│   ├── Config Tool (decrypt/encrypt)
│   ├── DNS Config [placeholder]
│   ├── TTL Masking [placeholder]
│   ├── Firewall [placeholder]
│   ├── Telemetry Block [placeholder]
│   ├── SSH Enabler [placeholder]
│   └── Device Explorer [placeholder]
└── Settings ──── Gateway IP, password, poll interval, theme
    └── Login (modal overlay on first launch / auth failure)
```

## Relation to CLI Toolkit

These mobile apps are companions to the Python CLI toolkit in the parent directory. The CLI tools use ADB/USB for full device access, while the mobile apps use WiFi/HTTP for features available through the ubus API.

| Capability | CLI (Python) | Mobile Apps |
|---|---|---|
| Signal monitoring | ADB ubus + AT commands + WiFi | WiFi HTTP only |
| Band locking | ADB ubus | WiFi HTTP |
| Config decrypt | Local file + Python crypto | On-device (CommonCrypto / javax.crypto) |
| SSH enabler | ADB (dropbear install) | Placeholder |
| TTL masking | ADB (iptables) | Placeholder |
| DNS config | ADB (resolv.conf) | Placeholder |
| Firewall | ADB (iptables) | Placeholder |
| Device explorer | ADB shell | Placeholder |

## License

Same as parent project.
