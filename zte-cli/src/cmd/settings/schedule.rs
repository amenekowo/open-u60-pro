use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show scheduled events
    Status(TransportArgs),
    /// Set scheduled reboot
    Reboot {
        #[command(flatten)]
        transport: TransportArgs,
        /// Enable scheduled reboot
        #[arg(long)]
        enable: bool,
        /// Disable scheduled reboot
        #[arg(long)]
        disable: bool,
        /// Time in HH:MM format
        #[arg(long)]
        time: Option<String>,
        /// Days (e.g. "1,2,3,4,5" for weekdays)
        #[arg(long)]
        days: Option<String>,
        #[arg(long)]
        confirm: bool,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Status(args) => {
            let t = get_transport(&args)?;
            let data = t.ubus_call("zwrt_bsp.power", "get_auto_reboot", None);
            if data == json!({}) {
                println!("{}", "No schedule data available.".yellow());
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            Ok(())
        }
        Cmd::Reboot { transport, enable, disable, time, days, confirm } => {
            let t = get_transport(&transport)?;
            if enable {
                confirm_guard(confirm, "enable scheduled reboot")?;
                let mut params = json!({"auto_reboot_enable": "1"});
                if let Some(ref tm) = time {
                    params["auto_reboot_time"] = json!(tm);
                }
                if let Some(ref d) = days {
                    params["auto_reboot_day"] = json!(d);
                }
                t.ubus_call("zwrt_bsp.power", "set_auto_reboot", Some(&params));
                println!("{}", "Scheduled reboot enabled.".green());
            } else if disable {
                confirm_guard(confirm, "disable scheduled reboot")?;
                t.ubus_call("zwrt_bsp.power", "set_auto_reboot", Some(&json!({"auto_reboot_enable": "0"})));
                println!("{}", "Scheduled reboot disabled.".green());
            } else {
                let data = t.ubus_call("zwrt_bsp.power", "get_auto_reboot", None);
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            Ok(())
        }
    }
}
