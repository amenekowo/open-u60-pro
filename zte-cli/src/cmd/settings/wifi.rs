use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::{Cell, Table};
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show WiFi configuration, channels, and client count
    Status(TransportArgs),
    /// Change WiFi settings (SSID, password, channel, power, etc.)
    Set {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)] ssid_2g: Option<String>,
        #[arg(long)] ssid_5g: Option<String>,
        #[arg(long)] password_2g: Option<String>,
        #[arg(long)] password_5g: Option<String>,
        #[arg(long)] channel_2g: Option<String>,
        #[arg(long)] channel_5g: Option<String>,
        #[arg(long)] txpower_2g: Option<String>,
        #[arg(long)] txpower_5g: Option<String>,
        #[arg(long)] encryption: Option<String>,
        #[arg(long)] encryption_2g: Option<String>,
        #[arg(long)] encryption_5g: Option<String>,
        #[arg(long)] radio_2g: Option<String>,
        #[arg(long)] radio_5g: Option<String>,
        #[arg(long)] hidden_2g: Option<String>,
        #[arg(long)] hidden_5g: Option<String>,
        #[arg(long)] htmode_2g: Option<String>,
        #[arg(long)] htmode_5g: Option<String>,
        #[arg(long)] wifi7: Option<String>,
        #[arg(long)] wifi_onoff: Option<String>,
        #[arg(long)] confirm: bool,
    },
    /// List connected WiFi clients
    Clients(TransportArgs),
    /// Kick a WiFi client by MAC address
    Kick {
        #[command(flatten)]
        transport: TransportArgs,
        mac: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Scan for nearby WiFi access points
    Scan(TransportArgs),
    /// Diagnose WiFi by comparing router config vs what macOS actually sees
    Diagnose(TransportArgs),
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Status(args) => {
            let t = get_transport(&args)?;
            // Try companion wifi_status first (works on both SSH and HTTP)
            let companion = t.ubus_call("zte-companion", "wifi_status", None);
            if companion != json!({}) && companion.get("error").is_none() {
                print_kv(
                    &companion,
                    &[
                        "wifi_onoff", "wifi6_switch", "ssid_2g", "ssid_5g",
                        "encryption_2g", "encryption_5g", "hidden_2g", "hidden_5g",
                        "channel_2g", "channel_5g", "actual_channel_2g", "actual_channel_5g",
                        "txpower_2g", "txpower_5g", "htmode_2g", "htmode_5g",
                        "country_code",
                    ],
                    Some("WiFi Status"),
                );
                if let Some(total) = companion.get("clients_total").and_then(|v| v.as_i64()) {
                    let c2g = companion.get("clients_2g").and_then(|v| v.as_i64()).unwrap_or(0);
                    let c5g = companion.get("clients_5g").and_then(|v| v.as_i64()).unwrap_or(0);
                    println!("\n{} {} (2.4GHz: {}, 5GHz: {})", "Connected Clients:".bold(), total, c2g, c5g);
                }
                return Ok(());
            }
            // Fallback to zwrt_wlan
            let status = t.ubus_call("zwrt_wlan", "status", None);
            if status != json!({}) {
                print_kv(
                    &status,
                    &[
                        "wifi_onoff", "wifi6_switch", "ssid_2g", "ssid_5g",
                        "encryption_2g", "encryption_5g", "hidden_2g", "hidden_5g",
                        "txpower_2g", "txpower_5g", "country_code",
                    ],
                    Some("WiFi Status"),
                );
            }
            let channels = t.ubus_call("zwrt_wlan", "get_current_channel", None);
            if channels != json!({}) {
                println!("\n{}", "Current Channels:".bold());
                println!("{}", serde_json::to_string_pretty(&channels)?);
            }
            let assoc = t.ubus_call("zwrt_wlan", "get_assoc_info", None);
            if let Some(clients) = assoc.get("assoc_list").or(assoc.get("sta_list")).and_then(|v| v.as_array()) {
                println!("\n{} {}", "Connected Clients:".bold(), clients.len());
            }
            Ok(())
        }
        Cmd::Set {
            transport, ssid_2g, ssid_5g, password_2g, password_5g,
            channel_2g, channel_5g, txpower_2g, txpower_5g, encryption,
            encryption_2g, encryption_5g, radio_2g, radio_5g,
            hidden_2g, hidden_5g, htmode_2g, htmode_5g, wifi7, wifi_onoff, confirm,
        } => {
            let mut params = serde_json::Map::new();
            if let Some(v) = ssid_2g { params.insert("ssid_2g".into(), json!(v)); }
            if let Some(v) = ssid_5g { params.insert("ssid_5g".into(), json!(v)); }
            if let Some(v) = password_2g { params.insert("key_2g".into(), json!(v)); }
            if let Some(v) = password_5g { params.insert("key_5g".into(), json!(v)); }
            if let Some(v) = channel_2g { params.insert("channel_2g".into(), json!(v)); }
            if let Some(v) = channel_5g { params.insert("channel_5g".into(), json!(v)); }
            if let Some(v) = txpower_2g { params.insert("txpower_2g".into(), json!(v)); }
            if let Some(v) = txpower_5g { params.insert("txpower_5g".into(), json!(v)); }
            if let Some(v) = encryption {
                params.insert("encryption_2g".into(), json!(v));
                params.insert("encryption_5g".into(), json!(v));
            }
            if let Some(v) = encryption_2g { params.insert("encryption_2g".into(), json!(v)); }
            if let Some(v) = encryption_5g { params.insert("encryption_5g".into(), json!(v)); }
            if let Some(v) = radio_2g { params.insert("radio2_disabled".into(), json!(v)); }
            if let Some(v) = radio_5g { params.insert("radio5_disabled".into(), json!(v)); }
            if let Some(v) = hidden_2g { params.insert("hidden_2g".into(), json!(v)); }
            if let Some(v) = hidden_5g { params.insert("hidden_5g".into(), json!(v)); }
            if let Some(v) = htmode_2g { params.insert("htmode_2g".into(), json!(v)); }
            if let Some(v) = htmode_5g { params.insert("htmode_5g".into(), json!(v)); }
            if let Some(v) = wifi7 { params.insert("wifi6_switch".into(), json!(v)); }
            if let Some(v) = wifi_onoff { params.insert("wifi_onoff".into(), json!(v)); }
            if params.is_empty() {
                println!("{}", "No settings specified. Use --help for options.".yellow());
                return Ok(());
            }
            // Warn if 160MHz bandwidth is paired with a UNII-3 channel (149-165)
            if let (Some(ch_val), Some(bw_val)) = (params.get("channel_5g"), params.get("htmode_5g")) {
                if bw_val.as_str() == Some("EHT160") {
                    if let Some(ch) = ch_val.as_str().and_then(|s| s.parse::<u32>().ok()) {
                        if (149..=165).contains(&ch) {
                            println!("{}", format!("Warning: Channel {} (UNII-3) supports max 80 MHz. For 160 MHz, use channels 36-64 or 100-128.", ch).yellow());
                        }
                    }
                }
            }
            confirm_guard(confirm, "apply WiFi changes")?;
            let t = get_transport(&transport)?;
            // Try companion wifi_set first
            let companion = t.ubus_call("zte-companion", "wifi_set", Some(&serde_json::Value::Object(params.clone())));
            if companion.get("status").and_then(|v| v.as_str()) == Some("ok") {
                let keys: Vec<&str> = params.keys().map(|s| s.as_str()).collect();
                println!("{}", format!("WiFi settings applied (companion): {}", keys.join(", ")).green());
                return Ok(());
            }
            // Fallback to zwrt_wlan set
            t.ubus_call("zwrt_wlan", "set", Some(&serde_json::Value::Object(params.clone())));
            let keys: Vec<&str> = params.keys().map(|s| s.as_str()).collect();
            println!("{}", format!("WiFi settings applied: {}", keys.join(", ")).green());
            Ok(())
        }
        Cmd::Clients(args) => {
            let t = get_transport(&args)?;
            let assoc = t.ubus_call("zwrt_wlan", "get_assoc_info", None);
            let clients = assoc.get("assoc_list").or(assoc.get("sta_list")).and_then(|v| v.as_array());
            match clients {
                Some(list) => {
                    let mut table = Table::new();
                    table.set_header(vec!["MAC", "IP", "Hostname", "Band", "RSSI"]);
                    for c in list {
                        let g = |k: &str| c.get(k).and_then(|v| v.as_str()).unwrap_or("--");
                        table.add_row(vec![g("mac"), g("ip"), g("hostname"), g("band"), g("rssi")]);
                    }
                    println!("{table}");
                }
                None => println!("{}", "No client info available.".yellow()),
            }
            Ok(())
        }
        Cmd::Kick { transport, mac, confirm } => {
            confirm_guard(confirm, &format!("kick {mac}"))?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_wlan", "kick_macs", Some(&json!({"mac_list": mac})));
            println!("{}", format!("Kicked client {mac}.").green());
            Ok(())
        }
        Cmd::Scan(args) => {
            let t = get_transport(&args)?;
            println!("Starting WiFi AP scan...");
            t.ubus_call("zwrt_wlan", "sta_start_scan", None);
            std::thread::sleep(std::time::Duration::from_secs(5));
            let results = t.ubus_call("zwrt_wlan", "get_scan_results", None);
            if results == json!({}) {
                println!("{}", "No scan results. Try again in a few seconds.".yellow());
            } else {
                println!("{}", serde_json::to_string_pretty(&results)?);
            }
            Ok(())
        }
        Cmd::Diagnose(args) => diagnose(args),
    }
}

// --- WiFi Diagnose ---

#[derive(Debug)]
struct CoreWlanAp {
    ssid: String,
    channel: i64,
    channel_width: i64,
    rssi: i64,
    bssid: String,
}

fn channel_width_str(raw: i64) -> &'static str {
    match raw {
        1 => "20 MHz",
        2 => "40 MHz",
        3 => "80 MHz",
        4 => "160 MHz",
        5 => "320 MHz",
        _ => "?",
    }
}

fn htmode_to_mhz(htmode: &str) -> &str {
    match htmode {
        s if s.contains("160") => "160 MHz",
        s if s.contains("80") => "80 MHz",
        s if s.contains("40") => "40 MHz",
        s if s.contains("20") => "20 MHz",
        _ => htmode,
    }
}

#[cfg(target_os = "macos")]
fn corewlan_scan() -> Result<Vec<CoreWlanAp>> {
    use objc2_core_wlan::CWWiFiClient;

    let client = unsafe { CWWiFiClient::sharedWiFiClient() };
    let iface = unsafe { client.interface() }
        .ok_or_else(|| anyhow::anyhow!("No WiFi interface found"))?;

    let networks = unsafe { iface.scanForNetworksWithName_error(None) }
        .map_err(|e| anyhow::anyhow!("CoreWLAN scan failed: {e}"))?;

    let mut aps = Vec::new();
    for network in &*networks {
        let ssid = unsafe { network.ssid() }
            .map(|s| s.to_string())
            .unwrap_or_default();
        if ssid.is_empty() {
            continue;
        }
        let (channel, width) = unsafe { network.wlanChannel() }
            .map(|ch| unsafe { (ch.channelNumber() as i64, ch.channelWidth().0 as i64) })
            .unwrap_or((0, 0));
        let rssi = unsafe { network.rssiValue() } as i64;
        let bssid = unsafe { network.bssid() }
            .map(|s| s.to_string())
            .unwrap_or_default();

        aps.push(CoreWlanAp {
            ssid,
            channel,
            channel_width: width,
            rssi,
            bssid,
        });
    }
    Ok(aps)
}

#[cfg(not(target_os = "macos"))]
fn corewlan_scan() -> Result<Vec<CoreWlanAp>> {
    anyhow::bail!("CoreWLAN scan is only available on macOS");
}

/// A single row in the diagnose table
struct DiagRow {
    metric: String,
    configured: String,
    actual: String,
    macos: String,
}

fn diagnose(args: TransportArgs) -> Result<()> {
    // Phase 1: Query router
    let t = get_transport(&args)?;
    let status = t.ubus_call("zte-companion", "wifi_status", None);
    if status == json!({}) || status.get("error").is_some() {
        anyhow::bail!("Failed to get wifi_status from zte-companion");
    }

    let s = |key: &str| -> String {
        status
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .unwrap_or("--")
            .to_string()
    };

    // Phase 2: CoreWLAN scan
    let macos_aps = if cfg!(target_os = "macos") {
        println!("Scanning nearby APs via CoreWLAN...");
        match corewlan_scan() {
            Ok(aps) => aps,
            Err(e) => {
                println!(
                    "{}",
                    format!("Warning: CoreWLAN scan failed: {e}. Continuing without macOS data.")
                        .yellow()
                );
                vec![]
            }
        }
    } else {
        println!(
            "{}",
            "Note: CoreWLAN scan is only available on macOS. Showing router data only.".yellow()
        );
        vec![]
    };

    // Match APs by SSID + band
    let ssid_5g = s("ssid_5g");
    let ssid_2g = s("ssid_2g");

    let find_ap = |ssid: &str, is_5g: bool| -> Option<&CoreWlanAp> {
        macos_aps.iter().find(|ap| {
            ap.ssid == ssid
                && if is_5g {
                    ap.channel >= 36
                } else {
                    ap.channel >= 1 && ap.channel <= 14
                }
        })
    };

    let ap_5g = find_ap(&ssid_5g, true);
    let ap_2g = find_ap(&ssid_2g, false);

    // Phase 3: Build rows
    let htmode_5g = s("htmode_5g");
    let htmode_2g = s("htmode_2g");

    let rows = vec![
        DiagRow {
            metric: "SSID 5G".into(),
            configured: ssid_5g.clone(),
            actual: "--".into(),
            macos: ap_5g.map(|a| a.ssid.clone()).unwrap_or("--".into()),
        },
        DiagRow {
            metric: "Channel 5G".into(),
            configured: s("channel_5g"),
            actual: s("actual_channel_5g"),
            macos: ap_5g
                .map(|a| a.channel.to_string())
                .unwrap_or("--".into()),
        },
        DiagRow {
            metric: "Bandwidth 5G".into(),
            configured: htmode_to_mhz(&htmode_5g).into(),
            actual: s("actual_bw_5g"),
            macos: ap_5g
                .map(|a| channel_width_str(a.channel_width).to_string())
                .unwrap_or("--".into()),
        },
        DiagRow {
            metric: "RSSI 5G".into(),
            configured: "--".into(),
            actual: "--".into(),
            macos: ap_5g
                .map(|a| format!("{} dBm", a.rssi))
                .unwrap_or("--".into()),
        },
        DiagRow {
            metric: "BSSID 5G".into(),
            configured: "--".into(),
            actual: "--".into(),
            macos: ap_5g.map(|a| a.bssid.clone()).unwrap_or("--".into()),
        },
        DiagRow {
            metric: "SSID 2G".into(),
            configured: ssid_2g.clone(),
            actual: "--".into(),
            macos: ap_2g.map(|a| a.ssid.clone()).unwrap_or("--".into()),
        },
        DiagRow {
            metric: "Channel 2G".into(),
            configured: s("channel_2g"),
            actual: s("actual_channel_2g"),
            macos: ap_2g
                .map(|a| a.channel.to_string())
                .unwrap_or("--".into()),
        },
        DiagRow {
            metric: "Bandwidth 2G".into(),
            configured: htmode_to_mhz(&htmode_2g).into(),
            actual: s("actual_bw_2g"),
            macos: ap_2g
                .map(|a| channel_width_str(a.channel_width).to_string())
                .unwrap_or("--".into()),
        },
        DiagRow {
            metric: "RSSI 2G".into(),
            configured: "--".into(),
            actual: "--".into(),
            macos: ap_2g
                .map(|a| format!("{} dBm", a.rssi))
                .unwrap_or("--".into()),
        },
    ];

    // Phase 4: Print table
    println!("\n{}", "WiFi Diagnostics".bold());
    let mut table = Table::new();
    table.set_header(vec!["Metric", "Configured (UCI)", "Actual (iw)", "macOS Sees"]);

    for row in &rows {
        table.add_row(vec![
            Cell::new(&row.metric),
            Cell::new(&row.configured),
            Cell::new(&row.actual),
            Cell::new(&row.macos),
        ]);
    }
    println!("{table}");

    // Phase 5: Mismatch summary
    let mut mismatches = Vec::new();

    for row in &rows {
        if !row.metric.starts_with("Channel") && !row.metric.starts_with("Bandwidth") {
            continue;
        }
        let configured = &row.configured;
        // Skip mismatch detection for "auto" channel or unavailable
        if configured == "auto" || configured == "--" {
            continue;
        }

        if row.actual != "--" && row.actual != *configured {
            mismatches.push(format!(
                "  ! {}: configured={} but iw reports {}",
                row.metric, configured, row.actual
            ));
        }
        if row.macos != "--" && row.macos != *configured {
            mismatches.push(format!(
                "  ! {}: configured={} but macOS sees {}",
                row.metric, configured, row.macos
            ));
        }
    }

    if !mismatches.is_empty() {
        println!("\n{}", "Mismatches detected:".red().bold());
        for m in &mismatches {
            println!("{}", m.yellow());
        }
        println!(
            "\n{}",
            "Tip: The router may have auto-selected a different channel due to DFS or interference."
                .cyan()
        );
    } else {
        println!("\n{}", "No mismatches detected.".green());
    }

    Ok(())
}
