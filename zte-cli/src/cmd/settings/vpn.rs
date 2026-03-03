use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show VPN status
    Status(TransportArgs),
    /// Enable VPN passthrough
    Enable {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Disable VPN passthrough
    Disable {
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
            let data = t.ubus_call("zwrt_router.api", "router_get_vpn_passthrough", None);
            if data == json!({}) {
                println!("{}", "No VPN data available.".yellow());
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            Ok(())
        }
        Cmd::Enable { transport, confirm } => {
            confirm_guard(confirm, "enable VPN passthrough")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_router.api", "router_set_vpn_passthrough", Some(&json!({
                "l2tp_passthrough": "1", "pptp_passthrough": "1", "ipsec_passthrough": "1",
            })));
            println!("{}", "VPN passthrough enabled.".green());
            Ok(())
        }
        Cmd::Disable { transport, confirm } => {
            confirm_guard(confirm, "disable VPN passthrough")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_router.api", "router_set_vpn_passthrough", Some(&json!({
                "l2tp_passthrough": "0", "pptp_passthrough": "0", "ipsec_passthrough": "0",
            })));
            println!("{}", "VPN passthrough disabled.".green());
            Ok(())
        }
    }
}
