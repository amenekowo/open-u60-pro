use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show LAN/DHCP configuration
    Status(TransportArgs),
    /// Set LAN IP and subnet
    SetIp {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        ip: String,
        #[arg(long, default_value = "255.255.255.0")]
        netmask: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Configure DHCP range
    Dhcp {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        start: String,
        #[arg(long)]
        end: String,
        #[arg(long, default_value = "86400")]
        lease_time: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Show connected LAN clients
    Clients(TransportArgs),
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Status(args) => {
            let t = get_transport(&args)?;
            let data = t.ubus_call("zwrt_router.api", "router_get_lan_para", None);
            print_kv(
                &data,
                &[
                    "lan_ipaddr", "lan_netmask", "dhcp_enable",
                    "dhcp_start", "dhcp_end", "dhcp_lease_time",
                ],
                Some("LAN/DHCP Configuration"),
            );
            Ok(())
        }
        Cmd::SetIp { transport, ip, netmask, confirm } => {
            confirm_guard(confirm, "change LAN IP")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_router.api", "router_set_lan_para", Some(&json!({
                "lan_ipaddr": ip,
                "lan_netmask": netmask,
            })));
            println!("{}", format!("LAN IP set to {ip}/{netmask}").green());
            Ok(())
        }
        Cmd::Dhcp { transport, start, end, lease_time, confirm } => {
            confirm_guard(confirm, "change DHCP range")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_router.api", "router_set_lan_para", Some(&json!({
                "dhcp_enable": "1",
                "dhcp_start": start,
                "dhcp_end": end,
                "dhcp_lease_time": lease_time,
            })));
            println!("{}", format!("DHCP range set to {start} - {end}").green());
            Ok(())
        }
        Cmd::Clients(args) => {
            let t = get_transport(&args)?;
            let data = t.ubus_call("zwrt_router.api", "router_get_connect_device_list", None);
            if data == json!({}) {
                println!("{}", "No client data available.".yellow());
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            Ok(())
        }
    }
}
