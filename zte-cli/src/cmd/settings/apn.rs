use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;
use serde_json::json;

use super::{confirm_guard, get_transport, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// List configured APN profiles
    List(TransportArgs),
    /// Add a new APN profile
    Add {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        name: String,
        #[arg(long)]
        apn: String,
        #[arg(long, default_value = "2")]
        pdp_type: String,
        #[arg(long, default_value = "0")]
        auth: String,
        #[arg(long, default_value = "")]
        user: String,
        #[arg(long, default_value = "")]
        password: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Delete an APN profile
    Delete {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        id: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Activate an APN profile
    Activate {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        id: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Switch between auto and manual APN mode
    Mode {
        #[command(flatten)]
        transport: TransportArgs,
        /// auto or manual
        mode: String,
        #[arg(long)]
        confirm: bool,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::List(args) => {
            let t = get_transport(&args)?;
            let mode = t.ubus_call("zwrt_apn_object", "get_apn_mode", None);
            if let Some(m) = mode.get("apn_mode").and_then(|v| v.as_str()) {
                let mode_str = if m == "1" { "manual" } else { "auto" };
                println!("{} {mode_str}", "APN mode:".bold());
            }
            let apns = t.ubus_call("zwrt_apn_object", "get_manu_apn_list", None);
            let list = apns
                .get("apn_list")
                .or_else(|| apns.get("manu_apn_list"))
                .and_then(|v| v.as_array());
            match list {
                Some(items) if !items.is_empty() => {
                    let mut table = Table::new();
                    table.set_header(vec!["ID", "Name", "APN", "PDP Type", "Auth", "Active"]);
                    fn pdp(v: &str) -> &'static str { match v { "0" => "IPv4", "1" => "IPv6", "2" | "3" => "IPv4v6", _ => "--" } }
                    fn auth(v: &str) -> &'static str { match v { "0" => "none", "1" => "PAP", "2" => "CHAP", _ => "--" } }
                    for a in items {
                        let id = a.get("id").or(a.get("apn_id")).and_then(|v| v.as_str()).unwrap_or("--");
                        table.add_row(vec![
                            id,
                            a.get("profilename").and_then(|v| v.as_str()).unwrap_or("--"),
                            a.get("wanapn").and_then(|v| v.as_str()).unwrap_or("--"),
                            pdp(&a.get("pdpType").map(|v| v.to_string()).unwrap_or_default()),
                            auth(&a.get("pppAuthMode").map(|v| v.to_string()).unwrap_or_default()),
                            a.get("active").and_then(|v| v.as_str()).unwrap_or("--"),
                        ]);
                    }
                    println!("{table}");
                }
                _ => println!("{}", "No APNs configured or unable to read.".yellow()),
            }
            Ok(())
        }
        Cmd::Add { transport, name, apn, pdp_type, auth, user, password, confirm } => {
            confirm_guard(confirm, "add APN profile")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_apn_object", "add_manu_apn", Some(&json!({
                "profilename": name,
                "wanapn": apn,
                "pdpType": pdp_type.parse::<i32>().unwrap_or(2),
                "pppAuthMode": auth.parse::<i32>().unwrap_or(0),
                "username": user,
                "password": password,
            })));
            println!("{}", format!("APN '{name}' ({apn}) added.").green());
            Ok(())
        }
        Cmd::Delete { transport, id, confirm } => {
            confirm_guard(confirm, "delete APN profile")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_apn_object", "delete_manu_apn", Some(&json!({"id": id})));
            println!("{}", format!("APN {id} deleted.").green());
            Ok(())
        }
        Cmd::Activate { transport, id, confirm } => {
            confirm_guard(confirm, "activate APN profile")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_apn_object", "enable_manu_apn_id", Some(&json!({"id": id})));
            println!("{}", format!("APN {id} activated.").green());
            Ok(())
        }
        Cmd::Mode { transport, mode, confirm } => {
            confirm_guard(confirm, &format!("set APN mode to {mode}"))?;
            let t = get_transport(&transport)?;
            let val = if mode == "auto" { "0" } else { "1" };
            t.ubus_call("zwrt_apn_object", "set_apn_mode", Some(&json!({"apn_mode": val})));
            println!("{}", format!("APN mode set to {mode}.").green());
            Ok(())
        }
    }
}
