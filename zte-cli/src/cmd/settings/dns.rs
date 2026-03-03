use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show current DNS configuration
    Status(TransportArgs),
    /// Set WAN DNS to manual mode with custom servers
    Set {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        primary: String,
        #[arg(long, default_value = "")]
        secondary: String,
        #[arg(long, default_value = "")]
        ipv6_primary: String,
        #[arg(long, default_value = "")]
        ipv6_secondary: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Restore DNS to auto mode (ISP-provided)
    Auto {
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
            let data = t.ubus_call("zwrt_router.api", "router_get_dns_para", None);
            print_kv(
                &data,
                &[
                    "dns_mode", "prefer_dns_manual", "standby_dns_manual",
                    "prefer_dns_auto", "standby_dns_auto",
                    "ipv6_prefer_dns_manual", "ipv6_standby_dns_manual",
                ],
                Some("DNS Configuration"),
            );
            Ok(())
        }
        Cmd::Set { transport, primary, secondary, ipv6_primary, ipv6_secondary, confirm } => {
            confirm_guard(confirm, "set DNS servers")?;
            let t = get_transport(&transport)?;
            let mut params = json!({
                "dns_mode": "manual",
                "prefer_dns_manual": primary,
                "standby_dns_manual": secondary,
            });
            if !ipv6_primary.is_empty() {
                params["ipv6_prefer_dns_manual"] = json!(ipv6_primary);
            }
            if !ipv6_secondary.is_empty() {
                params["ipv6_standby_dns_manual"] = json!(ipv6_secondary);
            }
            t.ubus_call("zwrt_router.api", "router_set_wan_dns", Some(&params));
            println!(
                "{}",
                format!("DNS set to {primary}{}", if secondary.is_empty() { String::new() } else { format!(" / {secondary}") }).green()
            );
            Ok(())
        }
        Cmd::Auto { transport, confirm } => {
            confirm_guard(confirm, "restore auto DNS")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_router.api", "router_set_wan_dns", Some(&json!({"dns_mode": "auto"})));
            println!("{}", "DNS restored to auto mode.".green());
            Ok(())
        }
    }
}
