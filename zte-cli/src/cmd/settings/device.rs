use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show device info (model, firmware, IMEI, SIM)
    Info(TransportArgs),
    /// Reboot the device
    Reboot {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Factory reset the device
    FactoryReset {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Set USB mode (debug/normal)
    Usb {
        #[command(flatten)]
        transport: TransportArgs,
        /// debug or normal
        mode: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Show battery and thermal status
    Battery(TransportArgs),
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Info(args) => {
            let t = get_transport(&args)?;
            let info = t.ubus_call("zwrt_bsp.info", "get_device_info", None);
            print_kv(
                &info,
                &[
                    "model", "firmware_version", "hardware_version",
                    "imei", "sim_status", "sim_iccid", "mac_addr",
                ],
                Some("Device Info"),
            );
            Ok(())
        }
        Cmd::Reboot { transport, confirm } => {
            confirm_guard(confirm, "reboot the device")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_bsp.power", "reboot", None);
            println!("{}", "Device is rebooting...".green());
            Ok(())
        }
        Cmd::FactoryReset { transport, confirm } => {
            confirm_guard(confirm, "factory reset the device")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_bsp.power", "factory_reset", None);
            println!("{}", "Factory reset initiated. Device will reboot.".green());
            Ok(())
        }
        Cmd::Usb { transport, mode, confirm } => {
            confirm_guard(confirm, &format!("set USB mode to {mode}"))?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_bsp.usb", "set", Some(&json!({"mode": mode})));
            println!("{}", format!("USB mode set to {mode}.").green());
            Ok(())
        }
        Cmd::Battery(args) => {
            let t = get_transport(&args)?;
            let bat = t.ubus_call("zwrt_bsp.battery", "list", None);
            print_kv(
                &bat,
                &[
                    "battery_capacity", "battery_temperature",
                    "battery_voltage", "battery_mode",
                    "battery_time_to_full", "battery_time_to_empty",
                    "charge_status", "charge_type",
                ],
                Some("Battery Status"),
            );
            let thermal = t.ubus_call("zwrt_bsp.thermal", "get_cpu_temp", None);
            print_kv(&thermal, &["cpuss_temp"], Some("Thermal"));
            Ok(())
        }
    }
}
