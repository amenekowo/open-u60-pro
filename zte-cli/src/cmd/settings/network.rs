use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show current network mode and registration
    Status(TransportArgs),
    /// Set network preference (case-sensitive)
    SetMode {
        #[command(flatten)]
        transport: TransportArgs,
        /// Mode: WL_AND_5G, Only_5G, Only_LTE, Only_WCDMA, Only_GSM
        mode: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Toggle SA mode (SA_ENABLE, SA_DISABLE, SA_NSA)
    Sa {
        #[command(flatten)]
        transport: TransportArgs,
        /// SA_ENABLE, SA_DISABLE, or SA_NSA
        setting: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Trigger manual network scan
    Scan(TransportArgs),
    /// Show mobile data status (connect mode, roaming, enabled)
    DataStatus(TransportArgs),
    /// Enable mobile data
    DataOn {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Disable mobile data
    DataOff {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Status(args) => {
            let t = get_transport(&args)?;
            let nw = t.ubus_call("zte_nwinfo_api", "nwinfo_get_netinfo", None);
            print_kv(
                &nw,
                &[
                    "operate_mode",
                    "net_select", "net_select_mode", "network_type",
                    "wan_active_band", "network_provider", "mcc", "mnc",
                    "nr5g_pci", "nr5g_rsrp", "nr5g_snr",
                    "lte_pci", "lte_rsrp", "lte_snr",
                    "roam_status", "wan_ipaddr",
                ],
                Some("Network Status"),
            );
            Ok(())
        }
        Cmd::SetMode { transport, mode, confirm } => {
            confirm_guard(confirm, &format!("switch to {mode}"))?;
            let t = get_transport(&transport)?;
            t.ubus_call(
                "zte_nwinfo_api",
                "nwinfo_set_netselect",
                Some(&json!({"net_select": mode})),
            );
            println!("{}", format!("Network mode set to {mode}").green());
            Ok(())
        }
        Cmd::Sa { transport, setting, confirm } => {
            confirm_guard(confirm, &format!("set SA to {setting}"))?;
            let t = get_transport(&transport)?;
            t.ubus_call(
                "zte_nwinfo_api",
                "nwinfo_set_nr5g_sa",
                Some(&json!({"sa_setting": setting.to_uppercase()})),
            );
            println!("{}", format!("SA mode set to {}", setting.to_uppercase()).green());
            Ok(())
        }
        Cmd::Scan(args) => {
            let t = get_transport(&args)?;
            println!("Scanning networks (this may take up to 60s)...");
            let result = t.ubus_call("zte_nwinfo_api", "nwinfo_manual_scan", None);
            if result.is_null() || result == json!({}) {
                println!("{}", "No scan results returned.".yellow());
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            Ok(())
        }
        Cmd::DataStatus(args) => {
            let t = get_transport(&args)?;
            let data = t.ubus_call("zwrt_data", "get_wwaniface", Some(&json!({"cid": 1})));
            print_kv(
                &data,
                &["connect_mode", "roam_enable", "enable", "connect_status"],
                Some("Mobile Data"),
            );
            Ok(())
        }
        Cmd::DataOn { transport, confirm } => {
            confirm_guard(confirm, "enable mobile data")?;
            let t = get_transport(&transport)?;
            let current = t.ubus_call("zwrt_data", "get_wwaniface", Some(&json!({"cid": 1})));
            let connect_mode = current.get("connect_mode").and_then(|v| v.as_i64()).unwrap_or(0);
            let roam_enable = current.get("roam_enable").and_then(|v| v.as_i64()).unwrap_or(0);
            t.ubus_call(
                "zwrt_data",
                "set_wwaniface",
                Some(&json!({
                    "cid": 1,
                    "connect_mode": connect_mode,
                    "roam_enable": roam_enable,
                    "enable": 1
                })),
            );
            println!("{}", "Mobile data enabled.".green());
            Ok(())
        }
        Cmd::DataOff { transport, confirm } => {
            confirm_guard(confirm, "disable mobile data")?;
            let t = get_transport(&transport)?;
            let current = t.ubus_call("zwrt_data", "get_wwaniface", Some(&json!({"cid": 1})));
            let connect_mode = current.get("connect_mode").and_then(|v| v.as_i64()).unwrap_or(0);
            let roam_enable = current.get("roam_enable").and_then(|v| v.as_i64()).unwrap_or(0);
            t.ubus_call(
                "zwrt_data",
                "set_wwaniface",
                Some(&json!({
                    "cid": 1,
                    "connect_mode": connect_mode,
                    "roam_enable": roam_enable,
                    "enable": 0
                })),
            );
            println!("{}", "Mobile data disabled.".green());
            Ok(())
        }
    }
}
