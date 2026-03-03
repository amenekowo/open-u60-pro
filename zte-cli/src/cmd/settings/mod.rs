pub mod network;
pub mod cell;
pub mod apn;
pub mod wifi;
pub mod dns;
pub mod firewall;
pub mod qos;
pub mod vpn;
pub mod lan;
pub mod device;
pub mod schedule;

use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;
use serde_json::Value;

use crate::cmd::ShellArgs;

#[derive(Subcommand)]
pub enum Cmd {
    /// Network mode selection and SA/NSA toggle
    #[command(subcommand)]
    Network(network::Cmd),
    /// Cell locking and neighbor scanning
    #[command(subcommand)]
    Cell(cell::Cmd),
    /// APN profile management
    #[command(subcommand)]
    Apn(apn::Cmd),
    /// WiFi configuration and client management
    #[command(subcommand)]
    Wifi(wifi::Cmd),
    /// DNS management via ubus API
    #[command(subcommand)]
    Dns(dns::Cmd),
    /// Firewall, port forwarding, DMZ, UPnP
    #[command(subcommand)]
    Firewall(firewall::Cmd),
    /// QoS / traffic shaping
    #[command(subcommand)]
    Qos(qos::Cmd),
    /// VPN client and server
    #[command(subcommand)]
    Vpn(vpn::Cmd),
    /// LAN / DHCP settings
    #[command(subcommand)]
    Lan(lan::Cmd),
    /// Device hardware controls (USB, reboot, factory reset)
    #[command(subcommand)]
    Device(device::Cmd),
    /// Scheduled events (reboot, wifi, mobile data)
    #[command(subcommand)]
    Schedule(schedule::Cmd),
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Network(c) => network::run(c),
        Cmd::Cell(c) => cell::run(c),
        Cmd::Apn(c) => apn::run(c),
        Cmd::Wifi(c) => wifi::run(c),
        Cmd::Dns(c) => dns::run(c),
        Cmd::Firewall(c) => firewall::run(c),
        Cmd::Qos(c) => qos::run(c),
        Cmd::Vpn(c) => vpn::run(c),
        Cmd::Lan(c) => lan::run(c),
        Cmd::Device(c) => device::run(c),
        Cmd::Schedule(c) => schedule::run(c),
    }
}

// ---------------------------------------------------------------------------
// Transport helpers (shared across settings subcommands)
// ---------------------------------------------------------------------------

#[derive(clap::Args, Clone)]
pub struct TransportArgs {
    /// Connection args (HTTP default, --ssh or --adb for shell)
    #[command(flatten)]
    pub shell: ShellArgs,
}

pub struct Transport(pub zte_lib::device::DeviceShell);

impl Transport {
    pub fn ubus_call(&self, obj: &str, method: &str, params: Option<&Value>) -> Value {
        self.0.ubus_call_quiet(obj, method, params)
    }

    /// Run a shell command on the device (requires SSH or ADB transport).
    pub fn shell(&self, cmd: &str) -> anyhow::Result<String> {
        Ok(self.0.shell(cmd, 10)?)
    }
}

pub fn get_transport(args: &TransportArgs) -> Result<Transport> {
    let dev = args.shell.connect()?;
    Ok(Transport(dev))
}

pub fn confirm_guard(confirm: bool, action: &str) -> Result<()> {
    if !confirm {
        println!("{}", format!("Use --confirm to {action}.").yellow());
        std::process::exit(0);
    }
    Ok(())
}

pub fn print_kv(data: &Value, keys: &[&str], title: Option<&str>) {
    let mut table = Table::new();
    if let Some(t) = title {
        table.set_header(vec![t, ""]);
    }
    for &key in keys {
        let val = data
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("--");
        table.add_row(vec![key, val]);
    }
    println!("{table}");
}

/// Macro that generates a simple status/set settings subcommand pair.
///
/// Usage:
/// ```
/// ubus_cmd! {
///     group_name = "dns",
///     status_obj = "zwrt_router.api",
///     status_method = "router_get_dns_para",
///     status_keys = ["dns_mode", "prefer_dns_manual"],
///     title = "DNS Configuration",
/// }
/// ```
#[macro_export]
macro_rules! ubus_cmd {
    (
        status_obj = $sobj:expr,
        status_method = $smethod:expr,
        status_keys = [$($key:expr),* $(,)?],
        title = $title:expr $(,)?
    ) => {
        pub fn show_status(transport: &$crate::cmd::settings::Transport) {
            let data = transport.ubus_call($sobj, $smethod, None);
            if data.is_null() || data == serde_json::json!({}) {
                println!("{}", "No data available.".yellow());
                return;
            }
            $crate::cmd::settings::print_kv(&data, &[$($key),*], Some($title));
        }
    };
}
