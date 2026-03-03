use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show QoS status
    Status(TransportArgs),
    /// Enable QoS
    Enable {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Disable QoS
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
            let data = t.ubus_call("zwrt_router.api", "router_get_qos_switch", None);
            print_kv(&data, &["qos_switch", "qos_mode"], Some("QoS Status"));
            Ok(())
        }
        Cmd::Enable { transport, confirm } => {
            confirm_guard(confirm, "enable QoS")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_router.api", "router_set_qos_switch", Some(&json!({"qos_switch": "1"})));
            println!("{}", "QoS enabled.".green());
            Ok(())
        }
        Cmd::Disable { transport, confirm } => {
            confirm_guard(confirm, "disable QoS")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_router.api", "router_set_qos_switch", Some(&json!({"qos_switch": "0"})));
            println!("{}", "QoS disabled.".green());
            Ok(())
        }
    }
}
