use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;
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
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Status(args) => {
            let t = get_transport(&args)?;
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
            channel_2g, channel_5g, txpower_2g, txpower_5g, encryption, confirm,
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
            if params.is_empty() {
                println!("{}", "No settings specified. Use --help for options.".yellow());
                return Ok(());
            }
            confirm_guard(confirm, "apply WiFi changes")?;
            let t = get_transport(&transport)?;
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
    }
}
