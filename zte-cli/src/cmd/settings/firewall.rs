use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;
use serde_json::json;

use super::{confirm_guard, get_transport, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show firewall, NAT, and port forwarding state
    Status(TransportArgs),
    /// Manage port forwarding rules
    PortForward {
        #[command(flatten)]
        transport: TransportArgs,
        /// List current rules
        #[arg(long)]
        list: bool,
        /// Enable port forwarding
        #[arg(long)]
        enable: bool,
        /// Disable port forwarding
        #[arg(long)]
        disable: bool,
        /// Add a port forward rule
        #[arg(long)]
        add: bool,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        proto: Option<String>,
        #[arg(long)]
        wan_port: Option<String>,
        #[arg(long)]
        lan_ip: Option<String>,
        #[arg(long)]
        lan_port: Option<String>,
        #[arg(long)]
        confirm: bool,
    },
    /// Configure DMZ
    Dmz {
        #[command(flatten)]
        transport: TransportArgs,
        /// Enable DMZ with this IP
        #[arg(long)]
        enable: Option<String>,
        /// Disable DMZ
        #[arg(long)]
        disable: bool,
        #[arg(long)]
        confirm: bool,
    },
    /// Toggle UPnP
    Upnp {
        #[command(flatten)]
        transport: TransportArgs,
        /// Enable UPnP
        #[arg(long)]
        enable: bool,
        /// Disable UPnP
        #[arg(long)]
        disable: bool,
        #[arg(long)]
        confirm: bool,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Status(args) => {
            let t = get_transport(&args)?;
            let mut table = Table::new();
            table.set_header(vec!["Setting", "Value"]);
            let fw = t.ubus_call("zwrt_router.api", "router_get_firewall_switch", None);
            if let Some(v) = fw.get("firewall_switch").and_then(|v| v.as_str()) {
                table.add_row(vec!["Firewall", v]);
            }
            let nat = t.ubus_call("zwrt_router.api", "router_get_nat_switch", None);
            if let Some(v) = nat.get("nat_switch").and_then(|v| v.as_str()) {
                table.add_row(vec!["NAT", v]);
            }
            let upnp = t.ubus_call("zwrt_router.api", "router_get_upnp_switch", None);
            if let Some(v) = upnp.get("enable_upnp").and_then(|v| v.as_str()) {
                table.add_row(vec!["UPnP", v]);
            }
            let dmz = t.ubus_call("zwrt_router.api", "router_get_dmz", None);
            if let Some(v) = dmz.get("dmz_enable").and_then(|v| v.as_str()) {
                table.add_row(vec!["DMZ", v]);
                if let Some(ip) = dmz.get("dmz_ip").and_then(|v| v.as_str()) {
                    table.add_row(vec!["DMZ IP", ip]);
                }
            }
            println!("{table}");
            Ok(())
        }
        Cmd::PortForward { transport, list, enable, disable, add, name, proto, wan_port, lan_ip, lan_port, confirm } => {
            let t = get_transport(&transport)?;
            if list {
                let data = t.ubus_call("zwrt_router.api", "router_get_portforward", None);
                println!("{}", serde_json::to_string_pretty(&data)?);
                return Ok(());
            }
            if enable {
                confirm_guard(confirm, "enable port forwarding")?;
                t.ubus_call("zwrt_router.api", "router_set_portforward_switch", Some(&json!({"portforward_switch": "1"})));
                println!("{}", "Port forwarding enabled.".green());
                return Ok(());
            }
            if disable {
                confirm_guard(confirm, "disable port forwarding")?;
                t.ubus_call("zwrt_router.api", "router_set_portforward_switch", Some(&json!({"portforward_switch": "0"})));
                println!("{}", "Port forwarding disabled.".green());
                return Ok(());
            }
            if add {
                let (wp, li, lp) = match (&wan_port, &lan_ip, &lan_port) {
                    (Some(a), Some(b), Some(c)) => (a, b, c),
                    _ => anyhow::bail!("--wan-port, --lan-ip, and --lan-port are required for --add."),
                };
                confirm_guard(confirm, "add port forward rule")?;
                t.ubus_call("zwrt_router.api", "router_set_portforward", Some(&json!({
                    "name": name.as_deref().unwrap_or("rule"),
                    "protocol": proto.as_deref().unwrap_or("tcp+udp"),
                    "wan_port": wp,
                    "lan_ip": li,
                    "lan_port": lp,
                    "enable": "1",
                })));
                println!("{}", format!("Port forward added: WAN {wp} -> {li}:{lp}").green());
                return Ok(());
            }
            println!("Use --list, --enable, --disable, or --add with port options.");
            Ok(())
        }
        Cmd::Dmz { transport, enable, disable, confirm } => {
            let t = get_transport(&transport)?;
            if let Some(ip) = enable {
                confirm_guard(confirm, "enable DMZ")?;
                t.ubus_call("zwrt_router.api", "router_set_dmz", Some(&json!({"dmz_enable": "1", "dmz_ip": ip})));
                println!("{}", format!("DMZ enabled for {ip}.").green());
            } else if disable {
                confirm_guard(confirm, "disable DMZ")?;
                t.ubus_call("zwrt_router.api", "router_set_dmz", Some(&json!({"dmz_enable": "0"})));
                println!("{}", "DMZ disabled.".green());
            } else {
                let data = t.ubus_call("zwrt_router.api", "router_get_dmz", None);
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            Ok(())
        }
        Cmd::Upnp { transport, enable, disable, confirm } => {
            let t = get_transport(&transport)?;
            if enable {
                confirm_guard(confirm, "enable UPnP")?;
                t.ubus_call("zwrt_router.api", "router_set_upnp_switch", Some(&json!({"enable_upnp": "1"})));
                println!("{}", "UPnP enabled.".green());
            } else if disable {
                confirm_guard(confirm, "disable UPnP")?;
                t.ubus_call("zwrt_router.api", "router_set_upnp_switch", Some(&json!({"enable_upnp": "0"})));
                println!("{}", "UPnP disabled.".green());
            } else {
                let data = t.ubus_call("zwrt_router.api", "router_get_upnp_switch", None);
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            Ok(())
        }
    }
}
